//! Structured, catalogue-backed diagnostic findings and artifact reports.
//!
//! The opening catalogue is intentionally small.  Its types are the common
//! representation used by later graph construction and validation modules.

use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, ConnectionKey,
    NetworkKey, NodeKey,
};
use crate::signal::SignalKind;
use core::cmp::Ordering;
use core::marker::PhantomData;

/// A stable subject to which a diagnostic condition applies.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl DiagnosticCode {
    /// Returns the stable dotted spelling of this catalogue entry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ValidationDuplicateKey => "validation.duplicate_key",
            Self::ValidationMissingNode => "validation.missing_node",
            Self::ValidationMissingPort => "validation.missing_port",
            Self::ValidationMissingEndpoint => "validation.missing_endpoint",
            Self::ValidationInvalidDirection => "validation.invalid_direction",
            Self::ValidationSignalKindMismatch => "validation.signal_kind_mismatch",
            Self::ValidationUnsupportedMultipleDrivers => "validation.unsupported_multiple_drivers",
            Self::ValidationMissingRequiredInput => "validation.missing_required_input",
            Self::ValidationInvalidFixedArity => "validation.invalid_fixed_arity",
            Self::InternalDiagnosticEvidenceConflict => "internal.diagnostic_evidence_conflict",
        }
    }

    const fn specification(self) -> CodeSpecification {
        const VALIDATION: CodeSpecification = CodeSpecification {
            severity: Severity::Error,
            responsibility: Responsibility::CallerInput,
            report_finding: true,
        };
        const DEFECT: CodeSpecification = CodeSpecification {
            severity: Severity::Error,
            responsibility: Responsibility::LibraryDefect,
            report_finding: false,
        };
        match self {
            Self::InternalDiagnosticEvidenceConflict => DEFECT,
            _ => VALIDATION,
        }
    }
}

#[derive(Clone, Copy)]
struct CodeSpecification {
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
        claims: Vec<SubjectRef>,
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
        ports: Vec<SubjectRef>,
        expected: usize,
        encountered: usize,
        marker: PhantomData<fn() -> D>,
    },
}

impl<D> ProblemEvidence<D> {
    /// Evidence for a duplicate structural key condition.
    #[must_use]
    pub fn duplicate_key(key: SubjectRef, claims: Vec<SubjectRef>) -> Self {
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
        ports: Vec<SubjectRef>,
        expected: usize,
        encountered: usize,
    ) -> Self {
        Self::ValidationInvalidFixedArity {
            ports,
            expected,
            encountered,
            marker: PhantomData,
        }
    }

    fn code(&self) -> DiagnosticCode {
        match self {
            Self::ValidationDuplicateKey { .. } => DiagnosticCode::ValidationDuplicateKey,
            Self::ValidationMissingNode { .. } => DiagnosticCode::ValidationMissingNode,
            Self::ValidationMissingPort { .. } => DiagnosticCode::ValidationMissingPort,
            Self::ValidationMissingEndpoint { .. } => DiagnosticCode::ValidationMissingEndpoint,
            Self::ValidationInvalidDirection { .. } => DiagnosticCode::ValidationInvalidDirection,
            Self::ValidationSignalKindMismatch { .. } => {
                DiagnosticCode::ValidationSignalKindMismatch
            }
            Self::ValidationUnsupportedMultipleDrivers { .. } => {
                DiagnosticCode::ValidationUnsupportedMultipleDrivers
            }
            Self::ValidationMissingRequiredInput { .. } => {
                DiagnosticCode::ValidationMissingRequiredInput
            }
            Self::ValidationInvalidFixedArity { .. } => DiagnosticCode::ValidationInvalidFixedArity,
        }
    }

    fn canonicalize(&mut self) {
        let canonicalize = |subjects: &mut Vec<SubjectRef>| {
            subjects.sort_by(SubjectRef::cmp_canonical);
            subjects.dedup();
        };
        match self {
            Self::ValidationDuplicateKey { claims, .. } => canonicalize(claims),
            Self::ValidationUnsupportedMultipleDrivers { drivers, .. } => canonicalize(drivers),
            Self::ValidationInvalidFixedArity { ports, .. } => canonicalize(ports),
            _ => {}
        }
    }
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
    pub fn new(
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
}

impl<D> DiagnosticSet<D> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            findings: Vec::new(),
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
    #[must_use]
    pub fn has_severity(&self, severity: Severity) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.problem().severity() == severity)
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
            if self.findings[existing] == diagnostic {
                return;
            }
            // The internal-defect delivery is deliberately not admitted to a report.
            // Keep the existing finding rather than letting detector order choose a value.
            return;
        }
        self.findings.push(diagnostic);
        self.findings.sort_by(compare_diagnostics);
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
    left.code == right.code && left.primary == right.primary
}
fn compare_diagnostics<D>(left: &Diagnostic<D>, right: &Diagnostic<D>) -> Ordering {
    let left = left.problem();
    let right = right.problem();
    left.severity()
        .rank()
        .cmp(&right.severity().rank())
        .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        .then_with(|| left.primary.cmp_canonical(&right.primary))
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
}
