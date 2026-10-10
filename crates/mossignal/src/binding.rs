//! Immutable adapters between caller-owned identifiers and stable external endpoints.

use crate::CompiledNetwork;
use crate::diagnostics::{
    BindingEvidence, BindingSubjectRef, DiagnosticCode, InputObservationEvidence,
    InspectionEvidence, InspectionSubjectKind, LifecycleEvidence, OperationSubjectRef, Problem,
    ProblemEvidence, RelatedSubject, RelatedSubjectRole, Report, Responsibility, Severity,
    SubjectRef,
};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint};
use crate::input::{InputBuildFailure, InputDelta, InputSnapshot};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, ExternalInputKey, ExternalOutputKey, NetworkKey,
};
use crate::machine::{Machine, NetworkRevision};
use crate::patch::{NetworkPatch, PreparedPatch};
use crate::policy::RuntimePolicy;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount, SignalKind, SignalType};
use crate::time::Time;
use crate::transaction::{CauseRef, OutputEvent, RuntimeFailure, Transaction, TransactionResult};
use core::fmt;
use core::marker::PhantomData;

#[derive(Clone)]
struct BindingContext {
    network: NetworkKey,
    fingerprint: NetworkFingerprint,
    input_schema: InputSchemaFingerprint,
}

/// A structured catalogue-backed application-binding failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingFailure {
    code: DiagnosticCode,
    evidence: Box<BindingEvidence>,
}

impl BindingFailure {
    fn new(code: DiagnosticCode, evidence: BindingEvidence) -> Self {
        Self {
            code,
            evidence: Box::new(evidence),
        }
    }

    /// Returns the stable catalogue code for this condition.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the catalogue-fixed severity.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.code.severity()
    }

    /// Returns the catalogue-fixed responsibility.
    #[must_use]
    pub const fn responsibility(&self) -> Responsibility {
        self.code.responsibility()
    }

    /// Returns the typed evidence carried by this failure.
    #[must_use]
    pub const fn evidence(&self) -> &BindingEvidence {
        &self.evidence
    }

    /// Converts this failure into the common catalogue-backed problem model.
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        let primary = self
            .evidence
            .endpoint
            .map(SubjectRef::Binding)
            .unwrap_or(SubjectRef::Network(self.evidence.network));
        let mut related = self
            .evidence
            .conflicting
            .iter()
            .copied()
            .map(|subject| RelatedSubject {
                role: RelatedSubjectRole::ConflictingClaim,
                subject: SubjectRef::Binding(subject),
            })
            .collect::<Vec<_>>();
        related.extend(
            self.evidence
                .missing
                .iter()
                .copied()
                .map(|subject| RelatedSubject {
                    role: RelatedSubjectRole::MissingReference,
                    subject: SubjectRef::Binding(subject),
                }),
        );
        let evidence = match self.code {
            DiagnosticCode::BindingUnknownEndpoint => ProblemEvidence::BindingUnknownEndpoint {
                evidence: (*self.evidence).clone(),
                marker: PhantomData,
            },
            DiagnosticCode::BindingWrongSignalKind => ProblemEvidence::BindingWrongSignalKind {
                evidence: (*self.evidence).clone(),
                marker: PhantomData,
            },
            DiagnosticCode::BindingDuplicateEndpoint => ProblemEvidence::BindingDuplicateEndpoint {
                evidence: (*self.evidence).clone(),
                marker: PhantomData,
            },
            DiagnosticCode::BindingDuplicateExternalKey => {
                ProblemEvidence::BindingDuplicateExternalKey {
                    evidence: (*self.evidence).clone(),
                    marker: PhantomData,
                }
            }
            DiagnosticCode::BindingAmbiguousExternalKey => {
                ProblemEvidence::BindingAmbiguousExternalKey {
                    evidence: (*self.evidence).clone(),
                    marker: PhantomData,
                }
            }
            DiagnosticCode::BindingMissingRequiredBinding => {
                ProblemEvidence::BindingMissingRequiredBinding {
                    evidence: (*self.evidence).clone(),
                    marker: PhantomData,
                }
            }
            DiagnosticCode::BindingWrongNetwork => ProblemEvidence::BindingWrongNetwork {
                evidence: (*self.evidence).clone(),
                marker: PhantomData,
            },
            DiagnosticCode::BindingStaleSchema => ProblemEvidence::BindingStaleSchema {
                evidence: (*self.evidence).clone(),
                marker: PhantomData,
            },
            DiagnosticCode::BindingInvalidReconfigurationContext => {
                ProblemEvidence::BindingInvalidReconfigurationContext {
                    evidence: (*self.evidence).clone(),
                    marker: PhantomData,
                }
            }
            _ => panic!("BindingFailure must contain a binding catalogue code"),
        };
        Problem::new(primary, related, evidence)
    }
}

impl fmt::Display for BindingFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl std::error::Error for BindingFailure {}

/// An immutable bidirectional mapping between stable endpoints and caller identifiers.
#[derive(Clone)]
pub struct BindingSet<I, O> {
    context: BindingContext,
    inputs: Vec<(AnyExternalInputKey, I)>,
    outputs: Vec<(AnyExternalOutputKey, O)>,
}

impl<I, O> BindingSet<I, O> {
    /// Starts a builder validated against one exact compiled topology.
    #[must_use]
    pub fn builder<D>(compiled: &CompiledNetwork<D>) -> BindingSetBuilder<'_, D, I, O> {
        BindingSetBuilder {
            compiled,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Returns the stable network identity retained by this adapter.
    #[must_use]
    pub const fn network_key(&self) -> NetworkKey {
        self.context.network
    }

    /// Returns the semantic topology identity retained by this adapter.
    #[must_use]
    pub const fn network_fingerprint(&self) -> NetworkFingerprint {
        self.context.fingerprint
    }

    /// Returns the exact input-schema identity retained by this adapter.
    #[must_use]
    pub const fn input_schema_fingerprint(&self) -> InputSchemaFingerprint {
        self.context.input_schema
    }
}

impl<I: Eq, O> BindingSet<I, O> {
    /// Looks up the stable input endpoint for one caller identifier.
    #[must_use]
    pub fn input_endpoint(&self, external: &I) -> Option<AnyExternalInputKey> {
        self.inputs
            .iter()
            .find_map(|(endpoint, candidate)| (candidate == external).then_some(*endpoint))
    }

    /// Looks up the caller identifier for one stable input endpoint.
    #[must_use]
    pub fn input_identifier<S: SignalType>(&self, endpoint: ExternalInputKey<S>) -> Option<&I> {
        let endpoint = erase_input(endpoint);
        self.inputs
            .iter()
            .find_map(|(candidate, external)| (*candidate == endpoint).then_some(external))
    }
}

impl<I, O: Eq> BindingSet<I, O> {
    /// Looks up the stable output endpoint for one caller identifier.
    #[must_use]
    pub fn output_endpoint(&self, external: &O) -> Option<AnyExternalOutputKey> {
        self.outputs
            .iter()
            .find_map(|(endpoint, candidate)| (candidate == external).then_some(*endpoint))
    }

    /// Looks up the caller identifier for one stable output endpoint.
    #[must_use]
    pub fn output_identifier<S: SignalType>(&self, endpoint: ExternalOutputKey<S>) -> Option<&O> {
        let endpoint = erase_output(endpoint);
        self.outputs
            .iter()
            .find_map(|(candidate, external)| (*candidate == endpoint).then_some(external))
    }
}

impl<I: Clone + Eq, O> BindingSet<I, O> {
    /// Creates an immutable complete input projector for this topology.
    pub fn input_projector<D>(
        &self,
        compiled: &CompiledNetwork<D>,
    ) -> Result<InputProjector<D, I>, BindingFailure> {
        validate_compiled(&self.context, compiled)?;
        let missing = compiled
            .graph()
            .external_inputs()
            .iter()
            .filter_map(|definition| {
                #[cfg(test)]
                crate::execution_work::update(|work| work.binding_slots_checked += 1);
                let endpoint = definition.key();
                (!self
                    .inputs
                    .iter()
                    .any(|(candidate, _)| *candidate == endpoint))
                .then_some(input_subject(&self.context, endpoint))
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(binding_failure(
                &self.context,
                DiagnosticCode::BindingMissingRequiredBinding,
                None,
                Vec::new(),
                missing,
                None,
            ));
        }
        Ok(InputProjector {
            compiled: compiled.clone(),
            inputs: self.inputs.clone(),
        })
    }
}

impl<I, O> BindingSet<I, O> {
    fn validate_complete<D>(&self, compiled: &CompiledNetwork<D>) -> Result<(), BindingFailure> {
        validate_compiled(&self.context, compiled)?;
        // Builder validation already establishes uniqueness and kind correctness;
        // private immutable mappings cannot acquire new endpoints after construction.
        let mut missing = compiled
            .graph()
            .external_inputs()
            .iter()
            .filter_map(|definition| {
                #[cfg(test)]
                crate::execution_work::update(|work| work.binding_slots_checked += 1);
                let endpoint = definition.key();
                (!self
                    .inputs
                    .iter()
                    .any(|(candidate, _)| *candidate == endpoint))
                .then_some(input_subject(&self.context, endpoint))
            })
            .collect::<Vec<_>>();
        missing.extend(
            compiled
                .graph()
                .external_outputs()
                .iter()
                .filter_map(|definition| {
                    #[cfg(test)]
                    crate::execution_work::update(|work| work.binding_slots_checked += 1);
                    let endpoint = definition.key();
                    (!self
                        .outputs
                        .iter()
                        .any(|(candidate, _)| *candidate == endpoint))
                    .then_some(output_subject(&self.context, endpoint))
                }),
        );
        if missing.is_empty() {
            Ok(())
        } else {
            Err(binding_failure(
                &self.context,
                DiagnosticCode::BindingMissingRequiredBinding,
                None,
                Vec::new(),
                missing,
                None,
            ))
        }
    }
}

/// A builder that validates application bindings against one compiled network.
pub struct BindingSetBuilder<'a, D, I, O> {
    compiled: &'a CompiledNetwork<D>,
    inputs: Vec<(AnyExternalInputKey, I)>,
    outputs: Vec<(AnyExternalOutputKey, O)>,
}

impl<'a, D, I: Eq, O: Eq> BindingSetBuilder<'a, D, I, O> {
    /// Adds one typed input binding.
    pub fn bind_input<S: SignalType>(
        mut self,
        endpoint: ExternalInputKey<S>,
        external: I,
    ) -> Result<Self, BindingFailure> {
        let context = context(self.compiled);
        let endpoint = erase_input(endpoint);
        validate_input_endpoint(self.compiled, &context, endpoint)?;
        let subject = input_subject(&context, endpoint);
        if self
            .inputs
            .iter()
            .any(|(candidate, _)| *candidate == endpoint)
        {
            return Err(binding_failure(
                &context,
                DiagnosticCode::BindingDuplicateEndpoint,
                Some(subject),
                vec![subject],
                Vec::new(),
                Some(endpoint.kind()),
            ));
        }
        if let Some((conflict, _)) = self
            .inputs
            .iter()
            .find(|(_, candidate)| *candidate == external)
        {
            return Err(binding_failure(
                &context,
                DiagnosticCode::BindingDuplicateExternalKey,
                Some(subject),
                vec![input_subject(&context, *conflict)],
                Vec::new(),
                Some(endpoint.kind()),
            ));
        }
        self.inputs.push((endpoint, external));
        Ok(self)
    }

    /// Adds one typed output binding.
    pub fn bind_output<S: SignalType>(
        mut self,
        endpoint: ExternalOutputKey<S>,
        external: O,
    ) -> Result<Self, BindingFailure> {
        let context = context(self.compiled);
        let endpoint = erase_output(endpoint);
        validate_output_endpoint(self.compiled, &context, endpoint)?;
        let subject = output_subject(&context, endpoint);
        if self
            .outputs
            .iter()
            .any(|(candidate, _)| *candidate == endpoint)
        {
            return Err(binding_failure(
                &context,
                DiagnosticCode::BindingDuplicateEndpoint,
                Some(subject),
                vec![subject],
                Vec::new(),
                Some(endpoint.kind()),
            ));
        }
        if let Some((conflict, _)) = self
            .outputs
            .iter()
            .find(|(_, candidate)| *candidate == external)
        {
            return Err(binding_failure(
                &context,
                DiagnosticCode::BindingDuplicateExternalKey,
                Some(subject),
                vec![output_subject(&context, *conflict)],
                Vec::new(),
                Some(endpoint.kind()),
            ));
        }
        self.outputs.push((endpoint, external));
        Ok(self)
    }

    /// Completes an immutable set. Completeness is checked by requested projectors or façades.
    pub fn finish(mut self) -> Result<BindingSet<I, O>, BindingFailure> {
        // SPEC: docs/specs/contracts/application-bindings.yaml
        // "caller-owned-nonsemantic-identifiers" — deterministic storage is keyed only by endpoints.
        self.inputs.sort_by_key(|(endpoint, _)| *endpoint);
        self.outputs.sort_by_key(|(endpoint, _)| *endpoint);
        Ok(BindingSet {
            context: context(self.compiled),
            inputs: self.inputs,
            outputs: self.outputs,
        })
    }
}

/// One caller-keyed observation accepted by an [`InputProjector`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputObservation<I> {
    Level { input: I, value: LogicLevel },
    Pulse { input: I, count: PulseCount },
}

/// A structured failure while projecting caller observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputProjectionFailure<I> {
    UnknownExternalKey {
        external: I,
    },
    WrongSignalKind {
        external: I,
        expected: SignalKind,
        actual: SignalKind,
    },
    InputBuild(InputBuildFailure),
}

impl<I> From<InputBuildFailure> for InputProjectionFailure<I> {
    fn from(value: InputBuildFailure) -> Self {
        Self::InputBuild(value)
    }
}

impl<I> InputProjectionFailure<I> {
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }
    /// Preserves an underlying input-build problem and otherwise projects the
    /// caller-key boundary leaf without treating the opaque caller key as core identity.
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        match self {
            Self::InputBuild(failure) => failure.problem(),
            Self::UnknownExternalKey { .. } => Problem::new(
                SubjectRef::Operation(OperationSubjectRef::InputProjection),
                Vec::new(),
                ProblemEvidence::InputUnknownEndpoint {
                    evidence: InputObservationEvidence {
                        endpoint: None,
                        expected_kind: None,
                        actual_kind: None,
                        observations: Vec::new(),
                        missing: Vec::new(),
                    },
                    marker: PhantomData,
                },
            ),
            Self::WrongSignalKind {
                expected, actual, ..
            } => Problem::new(
                SubjectRef::Operation(OperationSubjectRef::InputProjection),
                Vec::new(),
                ProblemEvidence::InputWrongSignalKind {
                    evidence: InputObservationEvidence {
                        endpoint: None,
                        expected_kind: Some(*expected),
                        actual_kind: Some(*actual),
                        observations: Vec::new(),
                        missing: Vec::new(),
                    },
                    marker: PhantomData,
                },
            ),
        }
    }
}

/// An immutable network-bound adapter from caller observations to canonical input artifacts.
pub struct InputProjector<D, I> {
    compiled: CompiledNetwork<D>,
    inputs: Vec<(AnyExternalInputKey, I)>,
}

impl<D, I: Eq + Clone> InputProjector<D, I> {
    /// Builds a complete ordinary input snapshot.
    pub fn snapshot_from(
        &self,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<InputSnapshot<D>, InputProjectionFailure<I>> {
        BorrowedInputs {
            compiled: &self.compiled,
            inputs: &self.inputs,
        }
        .snapshot_from(observations)
    }

    /// Builds an ordinary partial input delta.
    pub fn delta_from(
        &self,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<InputDelta<D>, InputProjectionFailure<I>> {
        BorrowedInputs {
            compiled: &self.compiled,
            inputs: &self.inputs,
        }
        .delta_from(observations)
    }
}

struct BorrowedInputs<'a, D, I> {
    compiled: &'a CompiledNetwork<D>,
    inputs: &'a [(AnyExternalInputKey, I)],
}

impl<D, I: Eq + Clone> BorrowedInputs<'_, D, I> {
    fn snapshot_from(
        &self,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<InputSnapshot<D>, InputProjectionFailure<I>> {
        let mut builder = self.compiled.input_snapshot();
        for observation in observations {
            match observation {
                InputObservation::Level { input, value } => {
                    let endpoint = lookup_input(self.inputs, &input).ok_or_else(|| {
                        InputProjectionFailure::UnknownExternalKey {
                            external: input.clone(),
                        }
                    })?;
                    let AnyExternalInputKey::Level(endpoint) = endpoint else {
                        return Err(InputProjectionFailure::WrongSignalKind {
                            external: input,
                            expected: SignalKind::Pulse,
                            actual: SignalKind::Level,
                        });
                    };
                    builder = builder.set(endpoint, value)?;
                }
                InputObservation::Pulse { input, count } => {
                    let endpoint = lookup_input(self.inputs, &input).ok_or_else(|| {
                        InputProjectionFailure::UnknownExternalKey {
                            external: input.clone(),
                        }
                    })?;
                    let AnyExternalInputKey::Pulse(endpoint) = endpoint else {
                        return Err(InputProjectionFailure::WrongSignalKind {
                            external: input,
                            expected: SignalKind::Level,
                            actual: SignalKind::Pulse,
                        });
                    };
                    builder = builder.pulse(endpoint, count)?;
                }
            }
        }
        builder.finish().map_err(Into::into)
    }

    fn delta_from(
        &self,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<InputDelta<D>, InputProjectionFailure<I>> {
        let mut builder = self.compiled.input_delta();
        for observation in observations {
            match observation {
                InputObservation::Level { input, value } => {
                    let endpoint = lookup_input(self.inputs, &input).ok_or_else(|| {
                        InputProjectionFailure::UnknownExternalKey {
                            external: input.clone(),
                        }
                    })?;
                    let AnyExternalInputKey::Level(endpoint) = endpoint else {
                        return Err(InputProjectionFailure::WrongSignalKind {
                            external: input,
                            expected: SignalKind::Pulse,
                            actual: SignalKind::Level,
                        });
                    };
                    builder = builder.set(endpoint, value)?;
                }
                InputObservation::Pulse { input, count } => {
                    let endpoint = lookup_input(self.inputs, &input).ok_or_else(|| {
                        InputProjectionFailure::UnknownExternalKey {
                            external: input.clone(),
                        }
                    })?;
                    let AnyExternalInputKey::Pulse(endpoint) = endpoint else {
                        return Err(InputProjectionFailure::WrongSignalKind {
                            external: input,
                            expected: SignalKind::Level,
                            actual: SignalKind::Pulse,
                        });
                    };
                    builder = builder.pulse(endpoint, count)?;
                }
            }
        }
        builder.finish().map_err(Into::into)
    }
}

/// A lossless caller-keyed projection of one ordinary output event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedOutputEvent<D, O> {
    LevelEstablished {
        endpoint: ExternalOutputKey<Level>,
        output: O,
        value: LogicLevel,
        stamp: crate::ReactionStamp<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    LevelChanged {
        endpoint: ExternalOutputKey<Level>,
        output: O,
        from: LogicLevel,
        to: LogicLevel,
        stamp: crate::ReactionStamp<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    Pulsed {
        endpoint: ExternalOutputKey<Pulse>,
        output: O,
        count: PulseCount,
        stamp: crate::ReactionStamp<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
}

impl<D, O> ProjectedOutputEvent<D, O> {
    /// Returns the stable endpoint in the producing core definition.
    #[must_use]
    pub fn endpoint(&self) -> AnyExternalOutputKey {
        match self {
            Self::LevelEstablished { endpoint, .. } | Self::LevelChanged { endpoint, .. } => {
                (*endpoint).into()
            }
            Self::Pulsed { endpoint, .. } => (*endpoint).into(),
        }
    }

    /// Returns the producing reaction occurrence.
    #[must_use]
    pub const fn stamp(&self) -> crate::ReactionStamp<D> {
        match self {
            Self::LevelEstablished { stamp, .. }
            | Self::LevelChanged { stamp, .. }
            | Self::Pulsed { stamp, .. } => *stamp,
        }
    }

    /// Returns the physical logical time of the producing occurrence.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.stamp().time()
    }
}

impl<I, O: Eq + Clone> BindingSet<I, O> {
    /// Projects one ordinary output event without altering any semantic field.
    ///
    /// The caller must select the bindings for the event's producing definition.
    /// An endpoint alone does not authenticate that definition. Bound operations
    /// select the source or target map automatically before publication.
    pub fn project_output_event<D>(
        &self,
        event: &OutputEvent<D>,
    ) -> Result<ProjectedOutputEvent<D, O>, BindingFailure> {
        match event {
            OutputEvent::LevelEstablished {
                output,
                value,
                stamp: at,
                cause,
                revision,
            } => Ok(ProjectedOutputEvent::LevelEstablished {
                endpoint: *output,
                output: self.required_output((*output).into())?,
                value: *value,
                stamp: *at,
                cause: *cause,
                revision: *revision,
            }),
            OutputEvent::LevelChanged {
                output,
                from,
                to,
                stamp: at,
                cause,
                revision,
            } => Ok(ProjectedOutputEvent::LevelChanged {
                endpoint: *output,
                output: self.required_output((*output).into())?,
                from: *from,
                to: *to,
                stamp: *at,
                cause: *cause,
                revision: *revision,
            }),
            OutputEvent::Pulsed {
                output,
                count,
                stamp: at,
                cause,
                revision,
            } => Ok(ProjectedOutputEvent::Pulsed {
                endpoint: *output,
                output: self.required_output((*output).into())?,
                count: *count,
                stamp: *at,
                cause: *cause,
                revision: *revision,
            }),
        }
    }

    fn required_output(&self, endpoint: AnyExternalOutputKey) -> Result<O, BindingFailure> {
        self.outputs
            .iter()
            .find_map(|(candidate, external)| (*candidate == endpoint).then(|| external.clone()))
            .ok_or_else(|| {
                let subject = output_subject(&self.context, endpoint);
                binding_failure(
                    &self.context,
                    DiagnosticCode::BindingMissingRequiredBinding,
                    Some(subject),
                    Vec::new(),
                    vec![subject],
                    Some(endpoint.kind()),
                )
            })
    }
}

/// A successful bound transaction containing the unchanged ordinary result and caller projection.
pub struct BoundTransactionResult<D, O> {
    ordinary: TransactionResult<D>,
    projected: Vec<ProjectedOutputEvent<D, O>>,
}

impl<D, O> BoundTransactionResult<D, O> {
    #[must_use]
    pub const fn ordinary(&self) -> &TransactionResult<D> {
        &self.ordinary
    }
    #[must_use]
    pub fn projected_output_events(&self) -> &[ProjectedOutputEvent<D, O>] {
        &self.projected
    }
    #[must_use]
    pub fn into_parts(self) -> (TransactionResult<D>, Vec<ProjectedOutputEvent<D, O>>) {
        (self.ordinary, self.projected)
    }
}

/// A failure from bound input projection or canonical machine execution.
#[derive(Debug)]
pub enum BoundApplyFailure<D, I> {
    Projection(InputProjectionFailure<I>),
    Runtime(RuntimeFailure<D>),
    Binding(BindingFailure),
}

impl<D, I> BoundApplyFailure<D, I> {
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        match self {
            Self::Projection(failure) => failure.code(),
            Self::Runtime(failure) => failure.code(),
            Self::Binding(failure) => failure.code(),
        }
    }
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }
    /// Returns an exact owned projection of the delegated source problem.
    #[must_use]
    pub fn problem(&self) -> Problem<D> {
        match self {
            Self::Projection(failure) => failure.problem(),
            Self::Runtime(failure) => failure.evidence().problem(),
            Self::Binding(failure) => failure.problem(),
        }
    }
}

/// A minimal ergonomic façade over one ordinary semantic machine.
pub struct BoundMachine<D, I, O> {
    machine: Machine<D>,
    bindings: BindingSet<I, O>,
}

impl<D, I: Eq + Clone, O: Eq + Clone> BoundMachine<D, I, O> {
    /// Combines a machine with complete bindings for its exact installed definition.
    /// Runtime revision and progress do not change binding compatibility.
    pub fn new(machine: Machine<D>, bindings: BindingSet<I, O>) -> Result<Self, BindingFailure> {
        bindings.validate_complete(machine.compiled())?;
        Ok(Self { machine, bindings })
    }

    /// Spawns and binds a fresh ordinary machine from the same compiled topology.
    pub fn spawn(
        compiled: &CompiledNetwork<D>,
        policy: RuntimePolicy,
        bindings: BindingSet<I, O>,
    ) -> Result<Self, BindingFailure> {
        Self::new(compiled.spawn(policy), bindings)
    }

    /// Inspects the machine without bypassing coherent mapping publication.
    #[must_use]
    pub const fn machine(&self) -> &Machine<D> {
        &self.machine
    }
    #[must_use]
    pub const fn bindings(&self) -> &BindingSet<I, O> {
        &self.bindings
    }

    fn borrowed_inputs(&self) -> Result<BorrowedInputs<'_, D, I>, BindingFailure> {
        validate_compiled(&self.bindings.context, self.machine.compiled())?;
        // SPEC: docs/specs/contracts/live-bindings.yaml "coherent-complete-ownership"
        // Construction/rebind/target publication prove this immutable mapping complete.
        Ok(BorrowedInputs {
            compiled: self.machine.compiled(),
            inputs: &self.bindings.inputs,
        })
    }

    /// Releases ownership of the machine and mappings for an explicit host transition.
    #[must_use]
    pub fn into_parts(self) -> (Machine<D>, BindingSet<I, O>) {
        (self.machine, self.bindings)
    }

    /// Replaces complete caller mappings for the exact installed definition.
    /// This produces no reaction and leaves every core freshness value unchanged.
    pub fn rebind(&mut self, bindings: BindingSet<I, O>) -> Result<(), BindingFailure> {
        bindings.validate_complete(self.machine.compiled())?;
        self.bindings = bindings;
        Ok(())
    }

    /// Prepares a topology replacement using the ordinary core validation path.
    pub fn prepare_patch(&self, patch: NetworkPatch<D>) -> Report<PreparedPatch<D>, D>
    where
        D: PartialEq,
    {
        self.machine.prepare_patch(patch)
    }

    /// Projects a complete caller snapshot and executes ordinary core initialization.
    pub fn initialize(
        &mut self,
        at: Time<D>,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        let input = self
            .borrowed_inputs()
            .map_err(BoundApplyFailure::Binding)?
            .snapshot_from(observations)
            .map_err(BoundApplyFailure::Projection)?;
        self.apply(Transaction::initialize(at, self.machine.revision(), input))
    }

    /// Projects a caller delta and executes ordinary core ready advancement.
    pub fn advance(
        &mut self,
        at: Time<D>,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        let input = self
            .borrowed_inputs()
            .map_err(BoundApplyFailure::Binding)?
            .delta_from(observations)
            .map_err(BoundApplyFailure::Projection)?;
        self.apply(Transaction::advance(at, self.machine.revision(), input))
    }

    /// Applies an ordinary transaction without a topology replacement.
    /// Expected revision, execution digest and lifecycle checks remain core controls.
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        self.apply_projecting(transaction, None, BindingSet::project_output_event)
    }

    /// Applies a patch-bearing transaction and complete target bindings atomically.
    /// Earlier deadlines retain source labels; the target reaction captures target labels.
    /// All structured failures leave both machine and mappings unchanged.
    pub fn apply_reconfigured(
        &mut self,
        transaction: Transaction<D>,
        target: BindingSet<I, O>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        self.apply_projecting(transaction, Some(target), BindingSet::project_output_event)
    }

    // The private projection parameter provides a focused fallible-publication test seam.
    // It is never exposed to hosts and is only called after core evaluation completes.
    fn apply_projecting(
        &mut self,
        transaction: Transaction<D>,
        target: Option<BindingSet<I, O>>,
        mut project: impl FnMut(
            &BindingSet<I, O>,
            &OutputEvent<D>,
        ) -> Result<ProjectedOutputEvent<D, O>, BindingFailure>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        if transaction.carries_patch() != target.is_some() {
            return Err(BoundApplyFailure::Binding(binding_failure(
                &self.bindings.context,
                DiagnosticCode::BindingInvalidReconfigurationContext,
                None,
                Vec::new(),
                Vec::new(),
                None,
            )));
        }
        validate_machine(&self.bindings.context, &self.machine)
            .map_err(BoundApplyFailure::Binding)?;
        if let (Some(bindings), Some(prepared)) = (&target, transaction.prepared_patch()) {
            bindings
                .validate_complete(prepared.resulting_compiled())
                .map_err(BoundApplyFailure::Binding)?;
        }
        let staged = self
            .machine
            .stage(transaction)
            .map_err(BoundApplyFailure::Runtime)?;
        let result = staged.result();
        // SPEC: docs/specs/contracts/live-bindings.yaml "historical-producing-map"
        // A removed endpoint may still emit at an earlier source deadline in this result.
        let projected = result
            .output_events()
            .iter()
            .map(|event| {
                let revision = match event {
                    OutputEvent::LevelEstablished { revision, .. }
                    | OutputEvent::LevelChanged { revision, .. }
                    | OutputEvent::Pulsed { revision, .. } => *revision,
                };
                let bindings = target
                    .as_ref()
                    .filter(|_| revision == result.after_revision())
                    .unwrap_or(&self.bindings);
                project(bindings, event)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(BoundApplyFailure::Binding)?;
        // SPEC: docs/specs/contracts/live-bindings.yaml "prepublication-projection"
        // Caller work finishes before the joint replacement, including before old maps drop.
        let ordinary = if let Some(bindings) = target {
            let (machine, result) = staged.into_parts();
            let _predecessor = core::mem::replace(self, Self { machine, bindings });
            result
        } else {
            staged.publish(&mut self.machine)
        };
        Ok(BoundTransactionResult {
            ordinary,
            projected,
        })
    }

    /// Returns one ready external Level output by caller identifier.
    /// The exact installed definition is checked independently of runtime revision.
    pub fn output_level(&self, external: &O) -> Result<LogicLevel, BoundOutputFailure> {
        // SPEC: docs/specs/contracts/application-bindings.yaml "immutable-compiled-schema-adapter"
        // Reads enforce the same topology binding as bound transactions.
        validate_machine(&self.bindings.context, &self.machine)
            .map_err(BoundOutputFailure::Binding)?;
        let Some(endpoint) = self.bindings.output_endpoint(external) else {
            return Err(BoundOutputFailure::UnknownExternalKey);
        };
        let AnyExternalOutputKey::Level(endpoint) = endpoint else {
            return Err(BoundOutputFailure::WrongSignalKind);
        };
        self.machine
            .output_level(endpoint)
            .ok_or(BoundOutputFailure::NotInitialized)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundOutputFailure {
    UnknownExternalKey,
    WrongSignalKind,
    NotInitialized,
    /// The binding set is incompatible with the machine's exact network definition.
    Binding(BindingFailure),
}

impl BoundOutputFailure {
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem::<()>().code()
    }
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }
    #[must_use]
    pub fn problem<D>(&self) -> Problem<D> {
        let requested = SubjectRef::Operation(OperationSubjectRef::OutputProjection);
        let evidence = match self {
            Self::Binding(failure) => return failure.problem(),
            Self::UnknownExternalKey => ProblemEvidence::InspectionUnknownSubject {
                evidence: InspectionEvidence {
                    requested: requested.clone(),
                    qualified_path: Vec::new(),
                    expected: InspectionSubjectKind::LevelOutput,
                    actual: None,
                },
                marker: PhantomData,
            },
            Self::WrongSignalKind => ProblemEvidence::InspectionWrongSubjectKind {
                evidence: InspectionEvidence {
                    requested: requested.clone(),
                    qualified_path: Vec::new(),
                    expected: InspectionSubjectKind::SignalKind(SignalKind::Level),
                    actual: Some(InspectionSubjectKind::SignalKind(SignalKind::Pulse)),
                },
                marker: PhantomData,
            },
            Self::NotInitialized => ProblemEvidence::LifecycleNotInitialized {
                evidence: LifecycleEvidence {
                    operation: OperationSubjectRef::OutputProjection,
                    current_time_ticks: None,
                },
                marker: PhantomData,
            },
        };
        Problem::new(requested, Vec::new(), evidence)
    }
}

fn context<D>(compiled: &CompiledNetwork<D>) -> BindingContext {
    BindingContext {
        network: compiled.network_key(),
        fingerprint: compiled.fingerprint(),
        input_schema: compiled.input_schema_fingerprint(),
    }
}

fn erase_input<S: SignalType>(endpoint: ExternalInputKey<S>) -> AnyExternalInputKey {
    match S::KIND {
        SignalKind::Level => ExternalInputKey::<Level>::from_u128(endpoint.as_u128()).into(),
        SignalKind::Pulse => ExternalInputKey::<Pulse>::from_u128(endpoint.as_u128()).into(),
    }
}

fn erase_output<S: SignalType>(endpoint: ExternalOutputKey<S>) -> AnyExternalOutputKey {
    match S::KIND {
        SignalKind::Level => ExternalOutputKey::<Level>::from_u128(endpoint.as_u128()).into(),
        SignalKind::Pulse => ExternalOutputKey::<Pulse>::from_u128(endpoint.as_u128()).into(),
    }
}

fn lookup_input<I: Eq>(
    bindings: &[(AnyExternalInputKey, I)],
    external: &I,
) -> Option<AnyExternalInputKey> {
    bindings
        .iter()
        .find_map(|(endpoint, candidate)| (candidate == external).then_some(*endpoint))
}

fn validate_input_endpoint<D>(
    compiled: &CompiledNetwork<D>,
    context: &BindingContext,
    endpoint: AnyExternalInputKey,
) -> Result<(), BindingFailure> {
    if compiled
        .graph()
        .external_inputs()
        .iter()
        .any(|definition| definition.key() == endpoint)
    {
        return Ok(());
    }
    let expected_kind = compiled
        .graph()
        .external_inputs()
        .iter()
        .find_map(|definition| {
            let candidate = definition.key();
            (input_payload(candidate) == input_payload(endpoint)).then_some(candidate.kind())
        });
    let subject = input_subject(context, endpoint);
    Err(binding_failure(
        context,
        if expected_kind.is_some() {
            DiagnosticCode::BindingWrongSignalKind
        } else {
            DiagnosticCode::BindingUnknownEndpoint
        },
        Some(subject),
        Vec::new(),
        Vec::new(),
        expected_kind,
    ))
}

fn validate_output_endpoint<D>(
    compiled: &CompiledNetwork<D>,
    context: &BindingContext,
    endpoint: AnyExternalOutputKey,
) -> Result<(), BindingFailure> {
    if compiled
        .graph()
        .external_outputs()
        .iter()
        .any(|definition| definition.key() == endpoint)
    {
        return Ok(());
    }
    let expected_kind = compiled
        .graph()
        .external_outputs()
        .iter()
        .find_map(|definition| {
            let candidate = definition.key();
            (output_payload(candidate) == output_payload(endpoint)).then_some(candidate.kind())
        });
    let subject = output_subject(context, endpoint);
    Err(binding_failure(
        context,
        if expected_kind.is_some() {
            DiagnosticCode::BindingWrongSignalKind
        } else {
            DiagnosticCode::BindingUnknownEndpoint
        },
        Some(subject),
        Vec::new(),
        Vec::new(),
        expected_kind,
    ))
}

fn validate_compiled<D>(
    context: &BindingContext,
    compiled: &CompiledNetwork<D>,
) -> Result<(), BindingFailure> {
    if context.network != compiled.network_key() {
        return Err(binding_failure(
            context,
            DiagnosticCode::BindingWrongNetwork,
            None,
            Vec::new(),
            Vec::new(),
            None,
        ));
    }
    if context.fingerprint != compiled.fingerprint()
        || context.input_schema != compiled.input_schema_fingerprint()
    {
        return Err(binding_failure(
            context,
            DiagnosticCode::BindingStaleSchema,
            None,
            Vec::new(),
            Vec::new(),
            None,
        ));
    }
    Ok(())
}

fn validate_machine<D>(
    context: &BindingContext,
    machine: &Machine<D>,
) -> Result<(), BindingFailure> {
    validate_compiled(context, machine.compiled())
}

fn input_subject(context: &BindingContext, endpoint: AnyExternalInputKey) -> BindingSubjectRef {
    BindingSubjectRef::Input {
        fingerprint: context.fingerprint,
        endpoint,
    }
}

fn output_subject(context: &BindingContext, endpoint: AnyExternalOutputKey) -> BindingSubjectRef {
    BindingSubjectRef::Output {
        fingerprint: context.fingerprint,
        endpoint,
    }
}

fn input_payload(endpoint: AnyExternalInputKey) -> u128 {
    match endpoint {
        AnyExternalInputKey::Level(key) => key.as_u128(),
        AnyExternalInputKey::Pulse(key) => key.as_u128(),
    }
}

fn output_payload(endpoint: AnyExternalOutputKey) -> u128 {
    match endpoint {
        AnyExternalOutputKey::Level(key) => key.as_u128(),
        AnyExternalOutputKey::Pulse(key) => key.as_u128(),
    }
}

fn binding_failure(
    context: &BindingContext,
    code: DiagnosticCode,
    endpoint: Option<BindingSubjectRef>,
    conflicting: Vec<BindingSubjectRef>,
    missing: Vec<BindingSubjectRef>,
    expected_kind: Option<SignalKind>,
) -> BindingFailure {
    let mut conflicting = conflicting;
    conflicting.sort();
    conflicting.dedup();
    let mut missing = missing;
    missing.sort();
    missing.dedup();
    BindingFailure::new(
        code,
        BindingEvidence {
            network: context.network,
            fingerprint: context.fingerprint,
            endpoint,
            conflicting,
            missing,
            expected_kind,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::ExternalOutputDef;
    use crate::metadata::DiagnosticMeta;
    use crate::time::NonZeroSpan;
    use crate::{NetworkBuilder, PulseDelayConfig, ReconfigurationPolicy, TimeDomainId};

    #[derive(Debug)]
    struct CountedKey {
        value: u64,
        clones: std::rc::Rc<std::cell::Cell<usize>>,
    }
    impl Clone for CountedKey {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                value: self.value,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for CountedKey {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }
    impl Eq for CountedKey {}

    #[test]
    fn published_reactions_borrow_complete_bindings_without_mapping_key_clones() {
        let clones = std::rc::Rc::new(std::cell::Cell::new(0));
        let key = |value| CountedKey {
            value,
            clones: clones.clone(),
        };
        let mut builder = NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
        let mut endpoints = Vec::new();
        for value in 1..=32 {
            let input = ExternalInputKey::<Level>::from_u128(value);
            let signal = builder
                .add_level_input(input, DiagnosticMeta::default())
                .unwrap();
            let output = ExternalOutputKey::<Level>::from_u128(100 + value);
            builder
                .add_level_output(output, signal, DiagnosticMeta::default())
                .unwrap();
            endpoints.push((input, output));
        }
        let compiled = builder
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let mut bindings = BindingSet::builder(&compiled);
        for (index, (input, output)) in endpoints.iter().copied().enumerate() {
            bindings = bindings
                .bind_input(input, key(index as u64))
                .unwrap()
                .bind_output(output, index as u64)
                .unwrap();
        }
        let policy = RuntimePolicy::builder()
            .max_internal_reactions(100)
            .max_evaluated_operations(10_000)
            .max_pending_events(100)
            .max_events_created_per_transaction(100)
            .max_required_provenance_growth(10_000)
            .build()
            .unwrap();
        let bindings = bindings.finish().unwrap();
        clones.set(0);
        crate::execution_work::reset();
        let owned = bindings.input_projector(&compiled).unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (32, 32)
        );
        clones.set(0);
        crate::execution_work::reset();
        let mut bound = BoundMachine::spawn(&compiled, policy, bindings).unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (0, 64)
        );
        clones.set(0);
        crate::execution_work::reset();
        bound
            .initialize(
                Time::from_ticks(0),
                (0..32).map(|index| InputObservation::Level {
                    input: key(index),
                    value: LogicLevel::Low,
                }),
            )
            .unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (0, 0)
        );
        for at in [1, 2, 40] {
            clones.set(0);
            crate::execution_work::reset();
            bound.advance(Time::from_ticks(at), []).unwrap();
            assert_eq!(
                (
                    clones.get(),
                    crate::execution_work::read().binding_slots_checked
                ),
                (0, 0)
            );
        }
        let before = bound.machine().snapshot();
        let mut replacement = BindingSet::builder(&compiled);
        for (index, (input, output)) in endpoints.iter().copied().enumerate() {
            replacement = replacement
                .bind_input(input, key(64 + index as u64))
                .unwrap()
                .bind_output(output, 64 + index as u64)
                .unwrap();
        }
        clones.set(0);
        crate::execution_work::reset();
        bound.rebind(replacement.finish().unwrap()).unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (0, 64)
        );
        assert_eq!(bound.machine().snapshot(), before);
        clones.set(0);
        crate::execution_work::reset();
        bound
            .advance(
                Time::from_ticks(41),
                [InputObservation::Level {
                    input: key(64),
                    value: LogicLevel::High,
                }],
            )
            .unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (0, 0)
        );
        // The standalone projector keeps its independent original owned map.
        let old = owned
            .delta_from([InputObservation::Level {
                input: key(0),
                value: LogicLevel::High,
            }])
            .unwrap();
        assert_eq!(
            old,
            compiled
                .input_delta()
                .set(endpoints[0].0, LogicLevel::High)
                .unwrap()
                .finish()
                .unwrap()
        );
        let prepared = bound
            .prepare_patch(
                bound
                    .machine()
                    .patch()
                    .set_diagnostic_meta(
                        crate::StructuralSubjectRef::Network(compiled.network_key()),
                        DiagnosticMeta {
                            name: Some("mapped".to_owned()),
                            ..DiagnosticMeta::default()
                        },
                    )
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let mut target = BindingSet::builder(prepared.resulting_compiled());
        for (index, (input, output)) in endpoints.iter().copied().enumerate() {
            target = target
                .bind_input(input, key(128 + index as u64))
                .unwrap()
                .bind_output(output, 128 + index as u64)
                .unwrap();
        }
        let target = target.finish().unwrap();
        let tx = Transaction::advance(
            Time::from_ticks(42),
            bound.machine().revision(),
            prepared.input_delta().finish().unwrap(),
        )
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
        clones.set(0);
        crate::execution_work::reset();
        bound.apply_reconfigured(tx, target).unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (0, 64)
        );
        clones.set(0);
        crate::execution_work::reset();
        let result = bound
            .advance(
                Time::from_ticks(43),
                [InputObservation::Level {
                    input: key(128),
                    value: LogicLevel::Low,
                }],
            )
            .unwrap();
        assert_eq!(
            (
                clones.get(),
                crate::execution_work::read().binding_slots_checked
            ),
            (0, 0)
        );
        assert!(matches!(
            result.projected_output_events(),
            [ProjectedOutputEvent::LevelChanged { output: 128, .. }]
        ));
    }

    #[test]
    fn borrowed_bound_inputs_preserve_owned_projector_rejections_without_mutation() {
        let mut b = NetworkBuilder::<()>::new(TimeDomainId::from_u128(2));
        let (level, signal) = b.level_input("level");
        let (pulse, _) = b.pulse_input("pulse");
        let output = b.level_output("level", signal).unwrap();
        let c = b
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let maps = BindingSet::builder(&c)
            .bind_input(level, "level")
            .unwrap()
            .bind_input(pulse, "pulse")
            .unwrap()
            .bind_output(output, "out")
            .unwrap()
            .finish()
            .unwrap();
        let owned = maps.input_projector(&c).unwrap();
        let policy = RuntimePolicy::builder()
            .max_internal_reactions(100)
            .max_evaluated_operations(1000)
            .max_pending_events(100)
            .max_events_created_per_transaction(100)
            .max_required_provenance_growth(1000)
            .build()
            .unwrap();
        for ready in [false, true] {
            let mut bound = BoundMachine::spawn(&c, policy.clone(), maps.clone()).unwrap();
            if ready {
                bound
                    .initialize(
                        Time::from_ticks(0),
                        [InputObservation::Level {
                            input: "level",
                            value: LogicLevel::Low,
                        }],
                    )
                    .unwrap();
            }
            let before = bound.machine().snapshot();
            let cases = [
                vec![],
                vec![InputObservation::Level {
                    input: "absent",
                    value: LogicLevel::Low,
                }],
                vec![InputObservation::Level {
                    input: "pulse",
                    value: LogicLevel::Low,
                }],
                vec![InputObservation::Pulse {
                    input: "level",
                    count: PulseCount::ONE,
                }],
                vec![
                    InputObservation::Level {
                        input: "level",
                        value: LogicLevel::Low,
                    },
                    InputObservation::Level {
                        input: "level",
                        value: LogicLevel::Low,
                    },
                ],
                vec![
                    InputObservation::Level {
                        input: "level",
                        value: LogicLevel::Low,
                    },
                    InputObservation::Level {
                        input: "level",
                        value: LogicLevel::High,
                    },
                ],
                vec![
                    InputObservation::Pulse {
                        input: "pulse",
                        count: PulseCount::ONE,
                    },
                    InputObservation::Pulse {
                        input: "pulse",
                        count: PulseCount::ONE,
                    },
                ],
                vec![
                    InputObservation::Pulse {
                        input: "pulse",
                        count: PulseCount::ONE,
                    },
                    InputObservation::Pulse {
                        input: "pulse",
                        count: PulseCount::new(2),
                    },
                ],
            ];
            for observations in cases {
                if ready && observations.is_empty() {
                    continue;
                }
                let expected = if ready {
                    owned.delta_from(observations.clone()).err().unwrap()
                } else {
                    owned.snapshot_from(observations.clone()).err().unwrap()
                };
                crate::execution_work::reset();
                let failure = if ready {
                    bound
                        .advance(Time::from_ticks(1), observations)
                        .err()
                        .unwrap()
                } else {
                    bound
                        .initialize(Time::from_ticks(0), observations)
                        .err()
                        .unwrap()
                };
                let BoundApplyFailure::Projection(actual) = failure else {
                    panic!("projection must reject before staging")
                };
                assert_eq!(actual, expected);
                let work = crate::execution_work::read();
                assert_eq!(work.binding_slots_checked, 0);
                assert_eq!(work.staged_stores_cloned, 0);
                assert_eq!(bound.machine().snapshot(), before);
            }
        }
    }

    #[test]
    fn late_projection_rejection_discards_the_complete_successor_and_target_maps() {
        let mut builder =
            NetworkBuilder::<()>::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let (input, pulse) = builder.pulse_input("trip");
        let delayed = builder
            .pulse_delay(
                pulse,
                PulseDelayConfig::new(NonZeroSpan::from_ticks(5).unwrap()),
            )
            .unwrap();
        let delay = builder.pulse_output("delayed", delayed).unwrap();
        let direct = builder.pulse_output("direct", pulse).unwrap();
        let compiled = builder
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let policy = RuntimePolicy::builder()
            .max_internal_reactions(100)
            .max_evaluated_operations(1000)
            .max_pending_events(100)
            .max_events_created_per_transaction(100)
            .max_required_provenance_growth(1000)
            .build()
            .unwrap();
        let bindings = BindingSet::builder(&compiled)
            .bind_input(input, "trip")
            .unwrap()
            .bind_output(delay, "old-delay")
            .unwrap()
            .bind_output(direct, "old-direct")
            .unwrap()
            .finish()
            .unwrap();
        let mut bound = BoundMachine::spawn(&compiled, policy, bindings).unwrap();
        bound
            .initialize(
                Time::from_ticks(0),
                [InputObservation::Pulse {
                    input: "trip",
                    count: PulseCount::ONE,
                }],
            )
            .unwrap();
        let new_delay = ExternalOutputKey::<Pulse>::from_u128(100);
        let source = compiled
            .graph()
            .external_outputs()
            .iter()
            .find(|o| o.key() == delay.into())
            .unwrap()
            .source();
        let prepared = bound
            .prepare_patch(
                bound
                    .machine()
                    .patch()
                    .remove_external_output(delay.into())
                    .unwrap()
                    .add_external_output(ExternalOutputDef::new(
                        new_delay.into(),
                        source,
                        DiagnosticMeta::default(),
                    ))
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let target = BindingSet::builder(prepared.resulting_compiled())
            .bind_input(input, "new-trip")
            .unwrap()
            .bind_output(new_delay, "new-delay")
            .unwrap()
            .bind_output(direct, "new-direct")
            .unwrap()
            .finish()
            .unwrap();
        let patched_input = prepared
            .input_delta()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap();
        let patched = Transaction::advance(
            Time::from_ticks(6),
            bound.machine().revision(),
            patched_input,
        )
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss)
        .unwrap();
        let ordinary_input = compiled
            .input_delta()
            .pulse(input, PulseCount::ONE)
            .unwrap()
            .finish()
            .unwrap();
        let ordinary = Transaction::advance(
            Time::from_ticks(6),
            bound.machine().revision(),
            ordinary_input,
        );
        let before = bound.machine().snapshot();
        let digest = bound.machine().execution_state_digest();
        for (transaction, target) in [(ordinary, None), (patched.clone(), Some(target.clone()))] {
            for reject_at in [1, 2] {
                let mut attempts = 0;
                crate::execution_work::reset();
                let failure = bound
                    .apply_projecting(transaction.clone(), target.clone(), |bindings, event| {
                        attempts += 1;
                        // Core evaluation has completed both deadlines and the requested reaction.
                        // In patch mode the second event already uses the new producing map/revision.
                        if event.at().ticks() == 5 {
                            assert_eq!(bindings.output_identifier(delay), Some(&"old-delay"));
                        } else if target.is_some() {
                            assert_eq!(bindings.output_identifier(direct), Some(&"new-direct"));
                        }
                        if attempts == reject_at {
                            let endpoint = match event {
                                OutputEvent::Pulsed { output, .. } => (*output).into(),
                                _ => panic!("fixture emits only pulses"),
                            };
                            let subject = output_subject(&bindings.context, endpoint);
                            Err(binding_failure(
                                &bindings.context,
                                DiagnosticCode::BindingMissingRequiredBinding,
                                Some(subject),
                                Vec::new(),
                                vec![subject],
                                Some(SignalKind::Pulse),
                            ))
                        } else {
                            bindings.project_output_event(event)
                        }
                    })
                    .err()
                    .unwrap();
                let work = crate::execution_work::read();
                assert_eq!(work.staged_stores_cloned, 1);
                assert_eq!(work.working_collections_cloned, 0);
                assert_eq!(
                    work.binding_slots_checked,
                    if target.is_some() { 3 } else { 0 }
                );
                assert_eq!(
                    failure.code(),
                    DiagnosticCode::BindingMissingRequiredBinding
                );
                assert_eq!(attempts, reject_at);
                assert_eq!(bound.machine().snapshot(), before);
                assert_eq!(bound.machine().execution_state_digest(), digest);
                assert_eq!(bound.bindings().input_identifier(input), Some(&"trip"));
                assert_eq!(
                    bound.bindings().output_identifier(delay),
                    Some(&"old-delay")
                );
                assert_eq!(
                    bound.bindings().output_identifier(direct),
                    Some(&"old-direct")
                );
                assert_eq!(bound.bindings().output_identifier(new_delay), None);
            }
        }
        let result = bound.apply_reconfigured(patched, target).unwrap();
        assert_eq!(result.projected_output_events().len(), 2);
        assert_eq!(
            bound.bindings().output_identifier(direct),
            Some(&"new-direct")
        );
    }
}
