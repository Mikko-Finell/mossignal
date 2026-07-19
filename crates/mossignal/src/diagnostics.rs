//! Structured, catalogue-backed diagnostic findings and artifact reports.
//!
//! The opening catalogue is intentionally small.  Its types are the common
//! representation used by later graph construction and validation modules.

use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, ConnectionKey,
    NetworkKey, NodeKey,
};
use crate::metadata::OriginRef;
use crate::signal::{LogicLevel, SignalKind};
use core::cmp::Ordering;
use core::marker::PhantomData;

/// A stable subject to which a diagnostic condition applies.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SubjectRef {
    /// An authored network.
    Network(NetworkKey),
    /// An authored node.
    Node(NodeKey),
    /// An authored node input port.
    InPort(AnyInPortKey),
    /// An authored node output port.
    OutPort(AnyOutPortKey),
    /// An authored connection.
    Connection(ConnectionKey),
    /// An authored external input.
    ExternalInput(AnyExternalInputKey),
    /// An authored external output.
    ExternalOutput(AnyExternalOutputKey),
}

impl SubjectRef {
    fn ordering_key(&self) -> (u8, SubjectPayload) {
        match self {
            Self::Network(key) => (0, SubjectPayload::Direct(key.as_u128())),
            Self::Node(key) => (7, SubjectPayload::Direct(key.as_u128())),
            Self::InPort(key) => (8, SubjectPayload::InPort(*key)),
            Self::OutPort(key) => (9, SubjectPayload::OutPort(*key)),
            Self::Connection(key) => (10, SubjectPayload::Direct(key.as_u128())),
            Self::ExternalInput(key) => (11, SubjectPayload::ExternalInput(*key)),
            Self::ExternalOutput(key) => (12, SubjectPayload::ExternalOutput(*key)),
        }
    }

    fn cmp_canonical(&self, other: &Self) -> Ordering {
        self.ordering_key().cmp(&other.ordering_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SubjectPayload {
    Direct(u128),
    InPort(AnyInPortKey),
    OutPort(AnyOutPortKey),
    ExternalInput(AnyExternalInputKey),
    ExternalOutput(AnyExternalOutputKey),
}

/// The severity fixed by a catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

/// The party or boundary responsible for a catalogue condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Responsibility {
    Advisory,
    CallerInput,
    SemanticRejection,
    Compatibility,
    ResourceLimit,
    CorruptData,
    UnsupportedFeature,
    ExternalIntegration,
    LibraryDefect,
}

/// A delivery form permitted by a catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemDelivery {
    ReportFinding,
    OperationFailure,
    InternalDefect,
}

/// The opening catalogue's structured identifiers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCode {
    ValidationDuplicateKey,
    ValidationMissingNode,
    ValidationMissingPort,
    ValidationMissingEndpoint,
    ValidationInvalidDirection,
    ValidationSignalKindMismatch,
    ValidationUnsupportedMultipleDrivers,
    ValidationMissingRequiredInput,
    ValidationInvalidFixedArity,
    InternalDiagnosticEvidenceConflict,
}

#[derive(Clone, Copy)]
struct CodeSpecification {
    code: &'static str,
    severity: Severity,
    responsibility: Responsibility,
    report_finding: bool,
}

/// Why a related semantic subject is included in a problem.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelatedSubjectRole {
    Source,
    Target,
    Owner,
    Driver,
    ConflictingDriver,
    ConflictingClaim,
    MissingReference,
    ExpectedSubject,
    ActualSubject,
    BaseSubject,
    TargetSubject,
    MigrationSource,
    MigrationTarget,
    CyclePredecessor,
    CycleSuccessor,
    Supporter,
    Blocker,
    InvalidatedArtifact,
}

/// A typed relationship to another subject involved in a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelatedSubject {
    pub role: RelatedSubjectRole,
    pub subject: SubjectRef,
}

/// The fixed node port group that an arity condition addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FixedArityRole {
    Input,
    Output,
}

/// A lossless projection of one authored claim that conflicts on a stable key.
///
/// This keeps duplicate-key evidence independent of caller record order while
/// retaining the authored facts needed to distinguish conflicting claims.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DuplicateClaim {
    Node {
        key: NodeKey,
        kind: DuplicateNodeKind,
        inputs: Vec<AnyInPortKey>,
        outputs: Vec<AnyOutPortKey>,
        origin: Option<OriginRef>,
    },
    InPort {
        key: AnyInPortKey,
        owner: NodeKey,
        origin: Option<OriginRef>,
    },
    OutPort {
        key: AnyOutPortKey,
        owner: NodeKey,
        origin: Option<OriginRef>,
    },
    Connection {
        key: ConnectionKey,
        source: SubjectRef,
        target: SubjectRef,
        origin: Option<OriginRef>,
    },
    ExternalInput {
        key: AnyExternalInputKey,
        origin: Option<OriginRef>,
    },
    ExternalOutput {
        key: AnyExternalOutputKey,
        source: SubjectRef,
        origin: Option<OriginRef>,
    },
}

/// The restricted node-kind facts retained by a duplicate node claim.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DuplicateNodeKind {
    Constant(LogicLevel),
    Not,
}

/// A safe, machine-readable correction.  The opening validation catalogue has
/// no unambiguous automatic correction, so no constructors are exposed yet.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suggestion {}

/// Structured evidence for one exact opening catalogue code.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemEvidence<D> {
    ValidationDuplicateKey {
        key: SubjectRef,
        claims: Vec<DuplicateClaim>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingNode {
        missing: NodeKey,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingPort {
        missing: SubjectRef,
        expected_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingEndpoint {
        missing: SubjectRef,
        expected_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidDirection {
        source: SubjectRef,
        target: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    ValidationSignalKindMismatch {
        source: SubjectRef,
        target: SubjectRef,
        source_kind: SignalKind,
        target_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationUnsupportedMultipleDrivers {
        drivers: Vec<SubjectRef>,
        marker: PhantomData<fn() -> D>,
    },
    ValidationMissingRequiredInput {
        required: SubjectRef,
        expected_kind: SignalKind,
        marker: PhantomData<fn() -> D>,
    },
    ValidationInvalidFixedArity {
        role: FixedArityRole,
        ports: Vec<SubjectRef>,
        expected: usize,
        encountered: usize,
        marker: PhantomData<fn() -> D>,
    },
    InternalDiagnosticEvidenceConflict {
        conflicting_code: DiagnosticCode,
        conflicting_primary: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
}

impl<D> ProblemEvidence<D> {
    /// Evidence for a duplicate structural key condition.
    #[must_use]
    pub fn duplicate_key(key: SubjectRef, claims: Vec<DuplicateClaim>) -> Self {
        Self::ValidationDuplicateKey {
            key,
            claims,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing node reference.
    #[must_use]
    pub fn missing_node(missing: NodeKey) -> Self {
        Self::ValidationMissingNode {
            missing,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing input or output port reference.
    #[must_use]
    pub fn missing_port(missing: SubjectRef, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingPort {
            missing,
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for a missing external endpoint reference.
    #[must_use]
    pub fn missing_endpoint(missing: SubjectRef, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingEndpoint {
            missing,
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for an invalid connection direction.
    #[must_use]
    pub fn invalid_direction(source: SubjectRef, target: SubjectRef) -> Self {
        Self::ValidationInvalidDirection {
            source,
            target,
            marker: PhantomData,
        }
    }

    /// Evidence for incompatible connection signal kinds.
    #[must_use]
    pub fn signal_kind_mismatch(
        source: SubjectRef,
        target: SubjectRef,
        source_kind: SignalKind,
        target_kind: SignalKind,
    ) -> Self {
        Self::ValidationSignalKindMismatch {
            source,
            target,
            source_kind,
            target_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for a target input's conflicting drivers.
    #[must_use]
    pub fn unsupported_multiple_drivers(drivers: Vec<SubjectRef>) -> Self {
        Self::ValidationUnsupportedMultipleDrivers {
            drivers,
            marker: PhantomData,
        }
    }

    /// Evidence for an absent required input.
    #[must_use]
    pub fn missing_required_input(required: SubjectRef, expected_kind: SignalKind) -> Self {
        Self::ValidationMissingRequiredInput {
            required,
            expected_kind,
            marker: PhantomData,
        }
    }

    /// Evidence for a fixed port-group arity mismatch.
    #[must_use]
    pub fn invalid_fixed_arity(
        role: FixedArityRole,
        ports: Vec<SubjectRef>,
        expected: usize,
        encountered: usize,
    ) -> Self {
        Self::ValidationInvalidFixedArity {
            role,
            ports,
            expected,
            encountered,
            marker: PhantomData,
        }
    }

    fn canonicalize(&mut self) {
        let canonicalize = |subjects: &mut Vec<SubjectRef>| {
            subjects.sort_by(SubjectRef::cmp_canonical);
            subjects.dedup();
        };
        match self {
            // SPEC: docs/specs/contracts/diagnostic-collections.yaml
            // "initial-duplicate-key" — claims are a multiset, not a set.
            Self::ValidationDuplicateKey { claims, .. } => claims.sort(),
            Self::ValidationUnsupportedMultipleDrivers { drivers, .. } => canonicalize(drivers),
            Self::ValidationInvalidFixedArity { ports, .. } => canonicalize(ports),
            _ => {}
        }
    }
}

// SPEC: docs/specs/contracts/diagnostic-problem-model.yaml
// "authoritative-registry" — code spelling, classification, delivery, and
// evidence association are declared together so they cannot drift apart.
macro_rules! opening_diagnostic_registry {
    ($( $code:ident, $evidence:pat, $spelling:literal, $severity:ident, $responsibility:ident, $report_finding:expr; )+) => {
        impl DiagnosticCode {
            /// Returns the stable dotted spelling of this catalogue entry.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                self.specification().code
            }

            const fn specification(self) -> CodeSpecification {
                match self {
                    $(Self::$code => CodeSpecification {
                        code: $spelling,
                        severity: Severity::$severity,
                        responsibility: Responsibility::$responsibility,
                        report_finding: $report_finding,
                    },)+
                }
            }
        }

        impl<D> ProblemEvidence<D> {
            fn code(&self) -> DiagnosticCode {
                match self {
                    $($evidence => DiagnosticCode::$code,)+
                }
            }
        }
    };
}

opening_diagnostic_registry! {
    ValidationDuplicateKey, Self::ValidationDuplicateKey { .. }, "validation.duplicate_key", Error, CallerInput, true;
    ValidationMissingNode, Self::ValidationMissingNode { .. }, "validation.missing_node", Error, CallerInput, true;
    ValidationMissingPort, Self::ValidationMissingPort { .. }, "validation.missing_port", Error, CallerInput, true;
    ValidationMissingEndpoint, Self::ValidationMissingEndpoint { .. }, "validation.missing_endpoint", Error, CallerInput, true;
    ValidationInvalidDirection, Self::ValidationInvalidDirection { .. }, "validation.invalid_direction", Error, CallerInput, true;
    ValidationSignalKindMismatch, Self::ValidationSignalKindMismatch { .. }, "validation.signal_kind_mismatch", Error, CallerInput, true;
    ValidationUnsupportedMultipleDrivers, Self::ValidationUnsupportedMultipleDrivers { .. }, "validation.unsupported_multiple_drivers", Error, CallerInput, true;
    ValidationMissingRequiredInput, Self::ValidationMissingRequiredInput { .. }, "validation.missing_required_input", Error, CallerInput, true;
    ValidationInvalidFixedArity, Self::ValidationInvalidFixedArity { .. }, "validation.invalid_fixed_arity", Error, CallerInput, true;
    InternalDiagnosticEvidenceConflict, Self::InternalDiagnosticEvidenceConflict { .. }, "internal.diagnostic_evidence_conflict", Error, LibraryDefect, false;
}

/// One structured, catalogue-valid problem record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem<D> {
    code: DiagnosticCode,
    primary: SubjectRef,
    related: Vec<RelatedSubject>,
    evidence: ProblemEvidence<D>,
    suggestions: Vec<Suggestion>,
}

impl<D> Problem<D> {
    /// Creates a problem only when its code and evidence are the catalogue pair.
    pub(crate) fn new(
        primary: SubjectRef,
        related: Vec<RelatedSubject>,
        mut evidence: ProblemEvidence<D>,
    ) -> Self {
        evidence.canonicalize();
        Self {
            code: evidence.code(),
            primary,
            related,
            evidence,
            suggestions: Vec::new(),
        }
    }

    fn evidence_conflict(
        conflicting_code: DiagnosticCode,
        conflicting_primary: SubjectRef,
    ) -> Self {
        Self::new(
            conflicting_primary,
            Vec::new(),
            ProblemEvidence::InternalDiagnosticEvidenceConflict {
                conflicting_code,
                conflicting_primary,
                marker: PhantomData,
            },
        )
    }
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.code.specification().severity
    }
    #[must_use]
    pub const fn responsibility(&self) -> Responsibility {
        self.code.specification().responsibility
    }
    #[must_use]
    pub const fn primary(&self) -> &SubjectRef {
        &self.primary
    }
    #[must_use]
    pub fn related(&self) -> &[RelatedSubject] {
        &self.related
    }
    #[must_use]
    pub const fn evidence(&self) -> &ProblemEvidence<D> {
        &self.evidence
    }
    #[must_use]
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }
}

/// A problem permitted as a report finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic<D> {
    problem: Problem<D>,
}

impl<D> Diagnostic<D> {
    /// Converts a catalogue-valid report problem into a report finding.
    pub fn new(problem: Problem<D>) -> Result<Self, Box<Problem<D>>> {
        if problem.code.specification().report_finding {
            Ok(Self { problem })
        } else {
            Err(Box::new(problem))
        }
    }
    #[must_use]
    pub const fn problem(&self) -> &Problem<D> {
        &self.problem
    }
    #[must_use]
    pub fn into_problem(self) -> Problem<D> {
        self.problem
    }
}

/// A deterministic owned collection of report findings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticSet<D> {
    findings: Vec<Diagnostic<D>>,
    internal_defects: Vec<Problem<D>>,
}

impl<D> DiagnosticSet<D> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            findings: Vec::new(),
            internal_defects: Vec::new(),
        }
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.findings.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic<D>> {
        self.findings.iter()
    }
    /// Returns internal invariant failures discovered while merging findings.
    #[must_use]
    pub fn internal_defects(&self) -> &[Problem<D>] {
        &self.internal_defects
    }
    #[must_use]
    pub fn has_severity(&self, severity: Severity) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.problem().severity() == severity)
            || self
                .internal_defects
                .iter()
                .any(|defect| defect.severity() == severity)
    }
    pub fn insert(&mut self, diagnostic: Diagnostic<D>)
    where
        D: PartialEq,
    {
        if let Some(existing) = self
            .findings
            .iter()
            .position(|old| same_condition(old.problem(), diagnostic.problem()))
        {
            let code = self.findings[existing].problem.code;
            let primary = self.findings[existing].problem.primary;
            let result = merge_evidence(
                &mut self.findings[existing].problem.evidence,
                diagnostic.problem.evidence,
            );
            match result {
                Ok(()) => {}
                Err(()) => self.record_evidence_conflict(code, primary),
            }
            self.findings.sort_by(compare_diagnostics);
            return;
        }
        self.findings.push(diagnostic);
        self.findings.sort_by(compare_diagnostics);
    }

    fn record_evidence_conflict(&mut self, code: DiagnosticCode, primary: SubjectRef) {
        if self.internal_defects.iter().any(|defect| {
            defect.code == DiagnosticCode::InternalDiagnosticEvidenceConflict
                && defect.primary == primary
                && matches!(
                    defect.evidence,
                    ProblemEvidence::InternalDiagnosticEvidenceConflict {
                        conflicting_code,
                        ..
                    } if conflicting_code == code
                )
        }) {
            return;
        }
        self.internal_defects
            .push(Problem::evidence_conflict(code, primary));
        self.internal_defects.sort_by(|left, right| {
            left.primary
                .cmp_canonical(&right.primary)
                .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        });
    }
}

impl<D> IntoIterator for DiagnosticSet<D> {
    type Item = Diagnostic<D>;
    type IntoIter = std::vec::IntoIter<Diagnostic<D>>;
    fn into_iter(self) -> Self::IntoIter {
        self.findings.into_iter()
    }
}

fn same_condition<D>(left: &Problem<D>, right: &Problem<D>) -> bool {
    left.code == right.code
        && left.primary == right.primary
        && condition_discriminator(&left.evidence) == condition_discriminator(&right.evidence)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConditionDiscriminator {
    Subject(SubjectRef),
    SubjectAndKind(SubjectRef, SignalKind),
    Subjects(SubjectRef, SubjectRef),
    Required(SubjectRef, SignalKind),
    Arity(FixedArityRole, usize, usize),
    Empty,
    Internal(DiagnosticCode, SubjectRef),
}

fn condition_discriminator<D>(evidence: &ProblemEvidence<D>) -> ConditionDiscriminator {
    match evidence {
        ProblemEvidence::ValidationDuplicateKey { key, .. } => {
            ConditionDiscriminator::Subject(*key)
        }
        ProblemEvidence::ValidationMissingNode { missing, .. } => {
            ConditionDiscriminator::Subject(SubjectRef::Node(*missing))
        }
        ProblemEvidence::ValidationMissingPort {
            missing,
            expected_kind,
            ..
        }
        | ProblemEvidence::ValidationMissingEndpoint {
            missing,
            expected_kind,
            ..
        } => ConditionDiscriminator::SubjectAndKind(*missing, *expected_kind),
        ProblemEvidence::ValidationInvalidDirection { source, target, .. } => {
            ConditionDiscriminator::Subjects(*source, *target)
        }
        ProblemEvidence::ValidationSignalKindMismatch { .. }
        | ProblemEvidence::ValidationUnsupportedMultipleDrivers { .. } => {
            ConditionDiscriminator::Empty
        }
        ProblemEvidence::ValidationMissingRequiredInput {
            required,
            expected_kind,
            ..
        } => ConditionDiscriminator::Required(*required, *expected_kind),
        ProblemEvidence::ValidationInvalidFixedArity {
            role,
            expected,
            encountered,
            ..
        } => ConditionDiscriminator::Arity(*role, *expected, *encountered),
        ProblemEvidence::InternalDiagnosticEvidenceConflict {
            conflicting_code,
            conflicting_primary,
            ..
        } => ConditionDiscriminator::Internal(*conflicting_code, *conflicting_primary),
    }
}

fn merge_evidence<D: PartialEq>(
    existing: &mut ProblemEvidence<D>,
    incoming: ProblemEvidence<D>,
) -> Result<(), ()> {
    match (existing, incoming) {
        (
            ProblemEvidence::ValidationUnsupportedMultipleDrivers { drivers, .. },
            ProblemEvidence::ValidationUnsupportedMultipleDrivers {
                drivers: incoming, ..
            },
        ) => {
            drivers.extend(incoming);
            drivers.sort_by(SubjectRef::cmp_canonical);
            drivers.dedup();
            Ok(())
        }
        (
            ProblemEvidence::ValidationInvalidFixedArity { ports, .. },
            ProblemEvidence::ValidationInvalidFixedArity {
                ports: incoming, ..
            },
        ) => {
            ports.extend(incoming);
            ports.sort_by(SubjectRef::cmp_canonical);
            ports.dedup();
            Ok(())
        }
        (existing, incoming) if *existing == incoming => Ok(()),
        _ => Err(()),
    }
}
fn compare_diagnostics<D>(left: &Diagnostic<D>, right: &Diagnostic<D>) -> Ordering {
    let left = left.problem();
    let right = right.problem();
    left.severity()
        .rank()
        .cmp(&right.severity().rank())
        .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        .then_with(|| left.primary.cmp_canonical(&right.primary))
        .then_with(|| {
            compare_discriminators(
                condition_discriminator(&left.evidence),
                condition_discriminator(&right.evidence),
            )
        })
}

fn compare_discriminators(left: ConditionDiscriminator, right: ConditionDiscriminator) -> Ordering {
    fn tag(value: ConditionDiscriminator) -> u8 {
        match value {
            ConditionDiscriminator::Subject(_) => 0,
            ConditionDiscriminator::SubjectAndKind(_, _) => 1,
            ConditionDiscriminator::Subjects(_, _) => 2,
            ConditionDiscriminator::Required(_, _) => 3,
            ConditionDiscriminator::Arity(_, _, _) => 4,
            ConditionDiscriminator::Empty => 5,
            ConditionDiscriminator::Internal(_, _) => 6,
        }
    }
    tag(left)
        .cmp(&tag(right))
        .then_with(|| match (left, right) {
            (ConditionDiscriminator::Subject(left), ConditionDiscriminator::Subject(right)) => {
                left.cmp_canonical(&right)
            }
            (
                ConditionDiscriminator::SubjectAndKind(left_subject, left_kind),
                ConditionDiscriminator::SubjectAndKind(right_subject, right_kind),
            ) => left_subject
                .cmp_canonical(&right_subject)
                .then_with(|| left_kind.cmp(&right_kind)),
            (
                ConditionDiscriminator::Subjects(left_first, left_second),
                ConditionDiscriminator::Subjects(right_first, right_second),
            ) => left_first
                .cmp_canonical(&right_first)
                .then_with(|| left_second.cmp_canonical(&right_second)),
            (
                ConditionDiscriminator::Required(left_subject, left_kind),
                ConditionDiscriminator::Required(right_subject, right_kind),
            ) => left_subject
                .cmp_canonical(&right_subject)
                .then_with(|| left_kind.cmp(&right_kind)),
            (
                ConditionDiscriminator::Arity(left_role, left_expected, left_encountered),
                ConditionDiscriminator::Arity(right_role, right_expected, right_encountered),
            ) => left_role
                .cmp(&right_role)
                .then_with(|| left_expected.cmp(&right_expected))
                .then_with(|| left_encountered.cmp(&right_encountered)),
            (
                ConditionDiscriminator::Internal(left_code, left_subject),
                ConditionDiscriminator::Internal(right_code, right_subject),
            ) => left_code
                .cmp(&right_code)
                .then_with(|| left_subject.cmp_canonical(&right_subject)),
            _ => Ordering::Equal,
        })
}

/// An artifact together with all independently collectable findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report<T, D> {
    artifact: Option<T>,
    diagnostics: DiagnosticSet<D>,
}

impl<T, D> Report<T, D> {
    /// Creates a report, suppressing the artifact when an error remains.
    #[must_use]
    pub fn new(artifact: Option<T>, diagnostics: DiagnosticSet<D>) -> Self {
        Self {
            artifact: if diagnostics.has_severity(Severity::Error) {
                None
            } else {
                artifact
            },
            diagnostics,
        }
    }
    #[must_use]
    pub const fn artifact(&self) -> Option<&T> {
        self.artifact.as_ref()
    }
    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticSet<D> {
        &self.diagnostics
    }
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_severity(Severity::Error)
    }
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.has_severity(Severity::Warning)
    }
    #[must_use]
    pub fn into_parts(self) -> (Option<T>, DiagnosticSet<D>) {
        (self.artifact, self.diagnostics)
    }
    pub fn require_artifact(self) -> Result<T, ReportFailure<D>> {
        match self.artifact {
            Some(value) => Ok(value),
            None => Err(ReportFailure {
                diagnostics: self.diagnostics,
            }),
        }
    }
}

/// A report without an artifact, retaining every collected finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFailure<D> {
    diagnostics: DiagnosticSet<D>,
}

impl<D> ReportFailure<D> {
    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticSet<D> {
        &self.diagnostics
    }
    #[must_use]
    pub fn into_diagnostics(self) -> DiagnosticSet<D> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::{AnyInPortKey, InPortKey};
    use crate::signal::Level;

    fn missing<D>(node: u128, missing: u128) -> Diagnostic<D> {
        Diagnostic::new(Problem::new(
            SubjectRef::Node(NodeKey::from_u128(node)),
            Vec::new(),
            ProblemEvidence::ValidationMissingNode {
                missing: NodeKey::from_u128(missing),
                marker: PhantomData,
            },
        ))
        .unwrap_or_else(|_| unreachable!("validation code is reportable"))
    }
    #[test]
    fn subject_order_is_tag_then_payload() {
        assert_eq!(
            SubjectRef::Network(NetworkKey::from_u128(99))
                .cmp_canonical(&SubjectRef::Node(NodeKey::from_u128(0))),
            Ordering::Less
        );
        assert_eq!(
            SubjectRef::InPort(AnyInPortKey::from(InPortKey::<Level>::from_u128(1))).cmp_canonical(
                &SubjectRef::InPort(AnyInPortKey::from(InPortKey::<Level>::from_u128(2)))
            ),
            Ordering::Less
        );
    }
    #[test]
    fn reports_retain_findings_and_block_errors() {
        let mut set = DiagnosticSet::new();
        set.insert(missing::<()>(1, 2));
        let report = Report::new(Some(7), set);
        assert_eq!(report.artifact(), None);
        assert_eq!(
            report
                .require_artifact()
                .err()
                .map(|failure| failure.diagnostics().len()),
            Some(1)
        );
    }
    #[test]
    fn equal_detection_deduplicates() {
        let mut set = DiagnosticSet::new();
        set.insert(missing::<()>(1, 2));
        set.insert(missing::<()>(1, 2));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn distinct_missing_references_on_one_owner_do_not_collapse() {
        let mut set = DiagnosticSet::new();
        set.insert(missing::<()>(1, 2));
        set.insert(missing::<()>(1, 3));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn duplicate_claims_remain_a_canonical_multiset() {
        let evidence = ProblemEvidence::<()>::duplicate_key(
            SubjectRef::Node(NodeKey::from_u128(7)),
            vec![
                DuplicateClaim::Node {
                    key: NodeKey::from_u128(3),
                    kind: DuplicateNodeKind::Not,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    origin: None,
                },
                DuplicateClaim::Node {
                    key: NodeKey::from_u128(3),
                    kind: DuplicateNodeKind::Not,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    origin: None,
                },
                DuplicateClaim::Node {
                    key: NodeKey::from_u128(2),
                    kind: DuplicateNodeKind::Not,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    origin: None,
                },
            ],
        );
        let problem = Problem::new(
            SubjectRef::Network(NetworkKey::from_u128(1)),
            Vec::new(),
            evidence,
        );
        match problem.evidence() {
            ProblemEvidence::ValidationDuplicateKey { claims, .. } => assert_eq!(
                claims,
                &[
                    DuplicateClaim::Node {
                        key: NodeKey::from_u128(2),
                        kind: DuplicateNodeKind::Not,
                        inputs: Vec::new(),
                        outputs: Vec::new(),
                        origin: None,
                    },
                    DuplicateClaim::Node {
                        key: NodeKey::from_u128(3),
                        kind: DuplicateNodeKind::Not,
                        inputs: Vec::new(),
                        outputs: Vec::new(),
                        origin: None,
                    },
                    DuplicateClaim::Node {
                        key: NodeKey::from_u128(3),
                        kind: DuplicateNodeKind::Not,
                        inputs: Vec::new(),
                        outputs: Vec::new(),
                        origin: None,
                    },
                ]
            ),
            _ => unreachable!("duplicate-key evidence was constructed"),
        }
    }

    #[test]
    fn driver_evidence_merges_as_a_canonical_set() {
        let primary = SubjectRef::InPort(AnyInPortKey::from(InPortKey::<Level>::from_u128(1)));
        let make = |drivers| {
            Diagnostic::new(Problem::new(
                primary,
                Vec::new(),
                ProblemEvidence::<()>::unsupported_multiple_drivers(drivers),
            ))
            .unwrap_or_else(|_| unreachable!("validation code is reportable"))
        };
        let mut set = DiagnosticSet::new();
        set.insert(make(vec![SubjectRef::Node(NodeKey::from_u128(3))]));
        set.insert(make(vec![
            SubjectRef::Node(NodeKey::from_u128(2)),
            SubjectRef::Node(NodeKey::from_u128(3)),
        ]));
        assert_eq!(set.len(), 1);
        match set
            .iter()
            .next()
            .map(Diagnostic::problem)
            .map(Problem::evidence)
        {
            Some(ProblemEvidence::ValidationUnsupportedMultipleDrivers { drivers, .. }) => {
                assert_eq!(
                    drivers,
                    &[
                        SubjectRef::Node(NodeKey::from_u128(2)),
                        SubjectRef::Node(NodeKey::from_u128(3)),
                    ]
                );
            }
            _ => unreachable!("merged driver evidence is retained"),
        }
    }

    #[test]
    fn contradictory_exact_evidence_records_an_internal_defect() {
        let primary = SubjectRef::Connection(ConnectionKey::from_u128(1));
        let make = |source| {
            Diagnostic::new(Problem::new(
                primary,
                Vec::new(),
                ProblemEvidence::<()>::signal_kind_mismatch(
                    source,
                    SubjectRef::Node(NodeKey::from_u128(2)),
                    SignalKind::Level,
                    SignalKind::Pulse,
                ),
            ))
            .unwrap_or_else(|_| unreachable!("validation code is reportable"))
        };
        let mut set = DiagnosticSet::new();
        set.insert(make(SubjectRef::Node(NodeKey::from_u128(3))));
        set.insert(make(SubjectRef::Node(NodeKey::from_u128(4))));
        assert_eq!(set.len(), 1);
        assert_eq!(set.internal_defects().len(), 1);
        assert_eq!(
            set.internal_defects()[0].code(),
            DiagnosticCode::InternalDiagnosticEvidenceConflict
        );
        assert_eq!(Report::new(Some(7), set).artifact(), None);
    }
}
