//! Immutable adapters between caller-owned identifiers and stable external endpoints.

use crate::CompiledNetwork;
use crate::diagnostics::{
    BindingEvidence, BindingSubjectRef, DiagnosticCode, Problem, ProblemEvidence, RelatedSubject,
    RelatedSubjectRole, Responsibility, Severity, SubjectRef,
};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint};
use crate::input::{InputBuildFailure, InputDelta, InputSnapshot};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, ExternalInputKey, ExternalOutputKey, NetworkKey,
};
use crate::machine::{Machine, NetworkRevision};
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
    revision: NetworkRevision,
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

    /// Returns the topology revision retained by this adapter.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.context.revision
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
        let mut builder = self.compiled.input_snapshot();
        for observation in observations {
            match observation {
                InputObservation::Level { input, value } => {
                    let endpoint = lookup_input(&self.inputs, &input).ok_or_else(|| {
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
                    let endpoint = lookup_input(&self.inputs, &input).ok_or_else(|| {
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

    /// Builds an ordinary partial input delta.
    pub fn delta_from(
        &self,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<InputDelta<D>, InputProjectionFailure<I>> {
        let mut builder = self.compiled.input_delta();
        for observation in observations {
            match observation {
                InputObservation::Level { input, value } => {
                    let endpoint = lookup_input(&self.inputs, &input).ok_or_else(|| {
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
                    let endpoint = lookup_input(&self.inputs, &input).ok_or_else(|| {
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
        output: O,
        value: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    LevelChanged {
        output: O,
        from: LogicLevel,
        to: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    Pulsed {
        output: O,
        count: PulseCount,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
}

impl<I, O: Eq + Clone> BindingSet<I, O> {
    /// Projects one ordinary output event without altering any semantic field.
    pub fn project_output_event<D>(
        &self,
        event: &OutputEvent<D>,
    ) -> Result<ProjectedOutputEvent<D, O>, BindingFailure> {
        match event {
            OutputEvent::LevelEstablished {
                output,
                value,
                at,
                cause,
                revision,
            } => Ok(ProjectedOutputEvent::LevelEstablished {
                output: self.required_output((*output).into())?,
                value: *value,
                at: *at,
                cause: *cause,
                revision: *revision,
            }),
            OutputEvent::LevelChanged {
                output,
                from,
                to,
                at,
                cause,
                revision,
            } => Ok(ProjectedOutputEvent::LevelChanged {
                output: self.required_output((*output).into())?,
                from: *from,
                to: *to,
                at: *at,
                cause: *cause,
                revision: *revision,
            }),
            OutputEvent::Pulsed {
                output,
                count,
                at,
                cause,
                revision,
            } => Ok(ProjectedOutputEvent::Pulsed {
                output: self.required_output((*output).into())?,
                count: *count,
                at: *at,
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

/// A minimal ergonomic façade over one ordinary semantic machine.
pub struct BoundMachine<D, I, O> {
    machine: Machine<D>,
    bindings: BindingSet<I, O>,
}

impl<D, I: Eq + Clone, O: Eq + Clone> BoundMachine<D, I, O> {
    /// Combines a machine with complete compatible input and output bindings.
    pub fn new(machine: Machine<D>, bindings: BindingSet<I, O>) -> Result<Self, BindingFailure> {
        validate_machine(&bindings.context, &machine)?;
        bindings.input_projector(machine.compiled())?;
        let missing = machine
            .compiled()
            .graph()
            .external_outputs()
            .iter()
            .filter_map(|definition| {
                let endpoint = definition.key();
                (!bindings
                    .outputs
                    .iter()
                    .any(|(candidate, _)| *candidate == endpoint))
                .then_some(output_subject(&bindings.context, endpoint))
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(binding_failure(
                &bindings.context,
                DiagnosticCode::BindingMissingRequiredBinding,
                None,
                Vec::new(),
                missing,
                None,
            ));
        }
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

    #[must_use]
    pub const fn machine(&self) -> &Machine<D> {
        &self.machine
    }
    #[must_use]
    pub fn machine_mut(&mut self) -> &mut Machine<D> {
        &mut self.machine
    }
    #[must_use]
    pub const fn bindings(&self) -> &BindingSet<I, O> {
        &self.bindings
    }

    /// Projects a complete caller snapshot and delegates initialization to `Machine::apply`.
    pub fn initialize(
        &mut self,
        at: Time<D>,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        let input = self
            .bindings
            .input_projector(self.machine.compiled())
            .map_err(BoundApplyFailure::Binding)?
            .snapshot_from(observations)
            .map_err(BoundApplyFailure::Projection)?;
        self.apply(Transaction::initialize(at, self.machine.revision(), input))
    }

    /// Projects a caller delta and delegates ready advancement to `Machine::apply`.
    pub fn advance(
        &mut self,
        at: Time<D>,
        observations: impl IntoIterator<Item = InputObservation<I>>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        let input = self
            .bindings
            .input_projector(self.machine.compiled())
            .map_err(BoundApplyFailure::Binding)?
            .delta_from(observations)
            .map_err(BoundApplyFailure::Projection)?;
        self.apply(Transaction::advance(at, self.machine.revision(), input))
    }

    fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<BoundTransactionResult<D, O>, BoundApplyFailure<D, I>> {
        validate_machine(&self.bindings.context, &self.machine)
            .map_err(BoundApplyFailure::Binding)?;
        // SPEC: docs/specs/contracts/application-bindings.yaml
        // "bound-machine-is-equivalent-delegation" — the façade never evaluates independently.
        let ordinary = self
            .machine
            .apply(transaction)
            .map_err(BoundApplyFailure::Runtime)?;
        let projected = ordinary
            .output_events()
            .iter()
            .map(|event| self.bindings.project_output_event(event))
            .collect::<Result<Vec<_>, _>>()
            .map_err(BoundApplyFailure::Binding)?;
        Ok(BoundTransactionResult {
            ordinary,
            projected,
        })
    }

    /// Returns one ready external Level output by caller identifier.
    pub fn output_level(&self, external: &O) -> Result<LogicLevel, BoundOutputFailure> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundOutputFailure {
    UnknownExternalKey,
    WrongSignalKind,
    NotInitialized,
}

fn context<D>(compiled: &CompiledNetwork<D>) -> BindingContext {
    BindingContext {
        network: compiled.network_key(),
        fingerprint: compiled.fingerprint(),
        input_schema: compiled.input_schema_fingerprint(),
        revision: NetworkRevision::initial(),
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
    let wrong_kind = compiled
        .graph()
        .external_inputs()
        .iter()
        .any(|definition| input_payload(definition.key()) == input_payload(endpoint));
    let subject = input_subject(context, endpoint);
    Err(binding_failure(
        context,
        if wrong_kind {
            DiagnosticCode::BindingWrongSignalKind
        } else {
            DiagnosticCode::BindingUnknownEndpoint
        },
        Some(subject),
        Vec::new(),
        Vec::new(),
        Some(endpoint.kind()),
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
    let wrong_kind = compiled
        .graph()
        .external_outputs()
        .iter()
        .any(|definition| output_payload(definition.key()) == output_payload(endpoint));
    let subject = output_subject(context, endpoint);
    Err(binding_failure(
        context,
        if wrong_kind {
            DiagnosticCode::BindingWrongSignalKind
        } else {
            DiagnosticCode::BindingUnknownEndpoint
        },
        Some(subject),
        Vec::new(),
        Vec::new(),
        Some(endpoint.kind()),
    ))
}

fn validate_compiled<D>(
    context: &BindingContext,
    compiled: &CompiledNetwork<D>,
) -> Result<(), BindingFailure> {
    if context.network != compiled.network_key() || context.fingerprint != compiled.fingerprint() {
        return Err(binding_failure(
            context,
            DiagnosticCode::BindingWrongNetwork,
            None,
            Vec::new(),
            Vec::new(),
            None,
        ));
    }
    if context.input_schema != compiled.input_schema_fingerprint() {
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
    validate_compiled(context, machine.compiled())?;
    if context.revision != machine.revision() {
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
            revision: context.revision,
            endpoint,
            conflicting,
            missing,
            expected_kind,
        },
    )
}
