//! Strict decoding and restoration of one canonical machine snapshot.
//!
//! Decoding accepts bytes. Restoration publishes one machine only after every
//! check has succeeded.

#![allow(clippy::result_large_err)]

use crate::cbor_decode::{self, DecodeError, Limits, Value};
use crate::compile::{CompiledNetwork, FullEvaluation, SnapshotNodeFamily};
use crate::diagnostics::{
    ArtifactIdentityEvidence, BudgetEvidence, CanonicalEncodingEvidence, ConflictControls,
    ConflictEvidence, DiagnosticCode, DiagnosticEpisodeEvidence, DigestCollisionEvidence,
    DigestMismatchEvidence, MissingSubjectEvidence, NodeEvidence, OperationSubjectRef,
    ParameterEvidence, PendingEventEvidence, PersistenceProvenanceEvidence, Problem,
    ProblemEvidence, Responsibility, SettledStateEvidence, Severity, StateSchemaEvidence,
    SubjectRef, VersionCompatibilityEvidence,
};
use crate::episode::{ActiveDiagnosticEpisode, DiagnosticConditionKey, DiagnosticEpisodeId};
use crate::identity::{
    ExecutionStateDigest, NetworkFingerprint, ObservableStateDigest, PROVENANCE_RECORD_DOMAIN,
    SNAPSHOT_DIGEST_DOMAIN, SnapshotDigest, TimeDomainId, domain_separated,
};
use crate::key::{ExternalInputKey, ExternalOutputKey, ModuleInstanceKey, NetworkKey, NodeKey};
use crate::machine::{
    Machine, MachineStatus, NetworkRevision, PendingEvent, PendingEventKey, PendingInertialDelay,
    PendingPeriodicBoundary, PendingPulseDelay, PendingTransportDelay,
};
use crate::module::{NodeSubject, PulsePortSubject, QualifiedNodeRef};
use crate::persistence::{MachineSnapshot, PersistenceContext, snapshot_digest_bytes};
use crate::policy::{RuntimePolicy, RuntimePolicyId};
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::standard::RetainedExpansionError;
use crate::state_digest::{cause_digest_index, encode_settled_endpoint};
use crate::time::Time;
use crate::transaction::{
    CauseRef, MIGRATED_CANCELLATION_RULE, ProvenanceRecord, ProvenanceSubject, ProvenanceView,
    PulseContribution,
};
use crate::{ConflictPolicy, EdgeObservation, FirstEmissionPolicy, ReenablePhasePolicy};
use core::fmt;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};

const ARTIFACT_PREFIX: [u8; 8] = [0x4d, 0x53, 0x49, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const SUPPORTED_VERSION: u64 = 2;

const ENVELOPE_FIELDS: &[&str] = &[
    "artifact_kind",
    "artifact_schema_version",
    "canonical_encoding_version",
    "core_semantics_version",
    "diagnostic_schema_version",
    "digest_suite_version",
    "envelope_schema_version",
    "integrity_digest",
    "node_semantics_version",
    "patch_semantics_version",
    "payload",
    "provenance_semantics_version",
    "time_domain_id",
];

const PAYLOAD_FIELDS: &[&str] = &[
    "active_diagnostic_episodes",
    "execution_state_digest",
    "lifecycle",
    "network_fingerprint",
    "network_key",
    "next_pending_event_serial",
    "node_state_table",
    "observable_state_digest",
    "optional_history",
    "persistence_metadata",
    "provenance",
    "runtime_policy_id",
    "semantic_versions",
    "temporal_state_table",
    "time_domain_id",
    "topology_revision",
];

const SEMANTIC_FIELDS: &[&str] = &[
    "core_semantics_version",
    "diagnostic_schema_version",
    "node_semantics_version",
    "patch_semantics_version",
    "provenance_semantics_version",
];

const NODE_KINDS: &[&str] = &[
    "all",
    "any",
    "any_edge",
    "at_least",
    "coalesce",
    "constant",
    "falling_edge",
    "inertial_delay",
    "level_set_reset_latch",
    "merge",
    "not",
    "parity",
    "periodic",
    "pulse_delay",
    "pulse_gate",
    "pulse_route",
    "pulse_select",
    "pulse_set_reset_latch",
    "rising_edge",
    "sample_hold",
    "select",
    "toggle",
    "transport_delay",
    "zip",
];

const STATE_SCHEMAS: &[&str] = &[
    "edge_observation",
    "pending_inertial_candidate",
    "pending_periodic_boundary",
    "pending_pulse_group",
    "pending_transport_transition",
    "periodic_anchor_previous_enable",
    "remembered_input_output",
    "stored_level",
];

const CHECKPOINTS: &[&str] = &[
    "authoritative_checkpoint",
    "checkpoint",
    "checkpoint_premise",
    "complete_from_checkpoint",
    "external_checkpoint",
];

/// Caller-supplied bounds for one snapshot decode.
///
/// Every limit is explicit. There is no hidden unbounded default. Ports,
/// connections, and replay frames are retained for the caller and are not
/// applied to machine-snapshot structure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct DecodePolicy {
    total_bytes: u64,
    nesting: u64,
    text_bytes: u64,
    byte_string_bytes: u64,
    collection_items: u64,
    nodes: u64,
    ports: u64,
    connections: u64,
    module_instances: u64,
    pending_events: u64,
    provenance_records: u64,
    provenance_edges: u64,
    diagnostic_records: u64,
    replay_frames: u64,
    optional_history_bytes: u64,
}

impl DecodePolicy {
    /// Builds a policy from the complete limit set.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        total_bytes: u64,
        nesting: u64,
        text_bytes: u64,
        byte_string_bytes: u64,
        collection_items: u64,
        nodes: u64,
        ports: u64,
        connections: u64,
        module_instances: u64,
        pending_events: u64,
        provenance_records: u64,
        provenance_edges: u64,
        diagnostic_records: u64,
        replay_frames: u64,
        optional_history_bytes: u64,
    ) -> Self {
        Self {
            total_bytes,
            nesting,
            text_bytes,
            byte_string_bytes,
            collection_items,
            nodes,
            ports,
            connections,
            module_instances,
            pending_events,
            provenance_records,
            provenance_edges,
            diagnostic_records,
            replay_frames,
            optional_history_bytes,
        }
    }

    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }
    #[must_use]
    pub const fn nesting(self) -> u64 {
        self.nesting
    }
    #[must_use]
    pub const fn text_bytes(self) -> u64 {
        self.text_bytes
    }
    #[must_use]
    pub const fn byte_string_bytes(self) -> u64 {
        self.byte_string_bytes
    }
    #[must_use]
    pub const fn collection_items(self) -> u64 {
        self.collection_items
    }
    #[must_use]
    pub const fn nodes(self) -> u64 {
        self.nodes
    }
    #[must_use]
    pub const fn ports(self) -> u64 {
        self.ports
    }
    #[must_use]
    pub const fn connections(self) -> u64 {
        self.connections
    }
    #[must_use]
    pub const fn module_instances(self) -> u64 {
        self.module_instances
    }
    #[must_use]
    pub const fn pending_events(self) -> u64 {
        self.pending_events
    }
    #[must_use]
    pub const fn provenance_records(self) -> u64 {
        self.provenance_records
    }
    #[must_use]
    pub const fn provenance_edges(self) -> u64 {
        self.provenance_edges
    }
    #[must_use]
    pub const fn diagnostic_records(self) -> u64 {
        self.diagnostic_records
    }
    #[must_use]
    pub const fn replay_frames(self) -> u64 {
        self.replay_frames
    }
    #[must_use]
    pub const fn optional_history_bytes(self) -> u64 {
        self.optional_history_bytes
    }

    fn for_recheck(length: u64) -> Self {
        Self::new(
            length, length, length, length, length, length, length, length, length, length, length,
            length, length, length, length,
        )
    }
}

/// Failure to decode one snapshot artifact.
///
/// Each leaf is a catalogue persistence condition. Decoding does not publish a
/// machine.
#[non_exhaustive]
pub enum DecodeFailure<D> {
    /// The fixed prefix is absent or wrong.
    InvalidPrefix(Problem<D>),
    /// The prefix or body ends before a complete value.
    TruncatedArtifact(Problem<D>),
    /// Bytes remain after one complete body.
    TrailingBytes(Problem<D>),
    /// The body is not canonical CBOR.
    NoncanonicalEncoding(Problem<D>),
    /// A known envelope or payload shape is incomplete or wrong.
    MalformedEnvelope(Problem<D>),
    /// The artifact kind string is not in the schema.
    UnknownArtifactKind(Problem<D>),
    /// A record names a field the schema does not declare.
    UnknownSchemaField(Problem<D>),
    /// A variant name is not in the schema.
    UnknownSchemaVariant(Problem<D>),
    /// The envelope integrity digest does not match.
    IntegrityDigestMismatch(Problem<D>),
    /// A caller decode limit was exceeded.
    DecodeLimitExceeded(Problem<D>),
    /// A version component is outside the supported vector.
    UnsupportedVersion(Problem<D>),
    /// The artifact time domain does not match the caller context.
    WrongTimeDomain(Problem<D>),
}

impl<D> DecodeFailure<D> {
    /// Returns the catalogue code for this leaf.
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem().code()
    }

    /// Returns the catalogue-fixed severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }

    /// Returns the catalogue-fixed responsibility.
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }

    /// Returns the catalogue-backed problem.
    #[must_use]
    pub fn problem(&self) -> &Problem<D> {
        match self {
            Self::InvalidPrefix(problem)
            | Self::TruncatedArtifact(problem)
            | Self::TrailingBytes(problem)
            | Self::NoncanonicalEncoding(problem)
            | Self::MalformedEnvelope(problem)
            | Self::UnknownArtifactKind(problem)
            | Self::UnknownSchemaField(problem)
            | Self::UnknownSchemaVariant(problem)
            | Self::IntegrityDigestMismatch(problem)
            | Self::DecodeLimitExceeded(problem)
            | Self::UnsupportedVersion(problem)
            | Self::WrongTimeDomain(problem) => problem,
        }
    }
}

impl<D> fmt::Debug for DecodeFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodeFailure")
            .field("code", &self.code().as_str())
            .finish()
    }
}

impl<D> fmt::Display for DecodeFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "snapshot decode failed with {}",
            self.code().as_str()
        )
    }
}

impl<D> std::error::Error for DecodeFailure<D> {}

/// Failure to restore one decoded snapshot onto a compiled network.
///
/// [`RestoreFailure::Decode`] delegates to the decode inventory. Every other
/// leaf is a restoration or standard-module condition. Failure publishes no
/// machine.
#[non_exhaustive]
pub enum RestoreFailure<D> {
    /// Framing, integrity, version, or decode-limit failure.
    Decode(DecodeFailure<D>),
    /// The artifact time domain is not the compiled network's domain.
    WrongTimeDomain(Problem<D>),
    /// The payload network key is not the compiled network key.
    NetworkIdentityMismatch(Problem<D>),
    /// The payload fingerprint is not the compiled fingerprint.
    FingerprintMismatch(Problem<D>),
    /// A persisted revision is newer than the snapshot topology revision.
    TopologyRevisionMismatch(Problem<D>),
    /// The supplied policy is not the snapshot policy.
    RuntimePolicyMismatch(Problem<D>),
    /// Ready and uninitialized facts contradict the lifecycle variant.
    LifecycleShapeInvalid(Problem<D>),
    /// A state owner or schema does not match the compiled topology.
    StateSchemaMismatch(Problem<D>),
    /// A persisted subject is not in the compiled topology.
    UnknownSubject(Problem<D>),
    /// A pending event's kind, payload, deadline, or cursor is invalid.
    PendingEventInvalid(Problem<D>),
    /// A singular temporal reference does not match the calendar.
    EventIdentityStateInvalid(Problem<D>),
    /// An episode's identity or evidence does not match its rule.
    DiagnosticEpisodeInvalid(Problem<D>),
    /// An episode code or schema is not the registered persistent form.
    DiagnosticSchemaInvalid(Problem<D>),
    /// Settled facts disagree with the reference evaluation.
    SettledStateInconsistent(Problem<D>),
    /// The recomputed execution-state digest disagrees.
    ExecutionDigestMismatch(Problem<D>),
    /// The recomputed observable-state digest disagrees.
    ObservableDigestMismatch(Problem<D>),
    /// The recomputed snapshot digest disagrees.
    SnapshotDigestMismatch(Problem<D>),
    /// Two different payloads share one content digest.
    DigestCollision(Problem<D>),
    /// A provenance record cites a digest that is not in the artifact.
    ProvenanceMissingPredecessor(Problem<D>),
    /// A provenance payload is not the canonical record for its kind.
    ProvenanceDigestMismatch(Problem<D>),
    /// The provenance graph contains a cycle.
    ProvenanceCycle(Problem<D>),
    /// A provenance subject does not resolve.
    ProvenanceInvalidSubject(Problem<D>),
    /// A predecessor role is not legal for the record.
    ProvenanceInvalidRole(Problem<D>),
    /// The record set is not the closure of the required roots.
    ProvenanceIncompleteRootClosure(Problem<D>),
    /// One digest is claimed by conflicting records or cause assignments.
    ProvenanceConflictingRecord(Problem<D>),
    /// The snapshot claims checkpoint authority it does not establish.
    ProvenanceFalseCheckpoint(Problem<D>),
    /// A retained standard declaration version is not in the current catalogue.
    StandardUnsupportedVersion(Problem<D>),
    /// A retained standard declaration does not match its interface.
    StandardInterfaceMismatch(Problem<D>),
    /// A retained standard expansion does not match the current descriptor.
    StandardExpansionMismatch(Problem<D>),
}

impl<D> RestoreFailure<D> {
    /// Returns the catalogue code for this leaf.
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem().code()
    }

    /// Returns the catalogue-fixed severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }

    /// Returns the catalogue-fixed responsibility.
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }

    /// Returns the catalogue-backed problem.
    #[must_use]
    pub fn problem(&self) -> &Problem<D> {
        match self {
            Self::Decode(failure) => failure.problem(),
            Self::WrongTimeDomain(problem)
            | Self::NetworkIdentityMismatch(problem)
            | Self::FingerprintMismatch(problem)
            | Self::TopologyRevisionMismatch(problem)
            | Self::RuntimePolicyMismatch(problem)
            | Self::LifecycleShapeInvalid(problem)
            | Self::StateSchemaMismatch(problem)
            | Self::UnknownSubject(problem)
            | Self::PendingEventInvalid(problem)
            | Self::EventIdentityStateInvalid(problem)
            | Self::DiagnosticEpisodeInvalid(problem)
            | Self::DiagnosticSchemaInvalid(problem)
            | Self::SettledStateInconsistent(problem)
            | Self::ExecutionDigestMismatch(problem)
            | Self::ObservableDigestMismatch(problem)
            | Self::SnapshotDigestMismatch(problem)
            | Self::DigestCollision(problem)
            | Self::ProvenanceMissingPredecessor(problem)
            | Self::ProvenanceDigestMismatch(problem)
            | Self::ProvenanceCycle(problem)
            | Self::ProvenanceInvalidSubject(problem)
            | Self::ProvenanceInvalidRole(problem)
            | Self::ProvenanceIncompleteRootClosure(problem)
            | Self::ProvenanceConflictingRecord(problem)
            | Self::ProvenanceFalseCheckpoint(problem)
            | Self::StandardUnsupportedVersion(problem)
            | Self::StandardInterfaceMismatch(problem)
            | Self::StandardExpansionMismatch(problem) => problem,
        }
    }
}

impl<D> fmt::Debug for RestoreFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestoreFailure")
            .field("code", &self.code().as_str())
            .finish()
    }
}

impl<D> fmt::Display for RestoreFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(failure) => write!(formatter, "{failure}"),
            _ => write!(
                formatter,
                "snapshot restoration failed with {}",
                self.code().as_str()
            ),
        }
    }
}

impl<D> std::error::Error for RestoreFailure<D> {}

/// Decodes one canonical machine snapshot.
///
/// The returned artifact keeps the original bytes, including optional history
/// and presentation metadata. Decoding does not restore a machine.
pub fn decode_snapshot<D>(
    context: &PersistenceContext<D>,
    bytes: &[u8],
    policy: &DecodePolicy,
) -> Result<MachineSnapshot<D>, DecodeFailure<D>> {
    let artifact = check_artifact(bytes, policy)?;
    if artifact.time_domain != context.time_domain() {
        return Err(DecodeFailure::WrongTimeDomain(identity_problem(
            "decode",
            &hex(&context.time_domain().to_be_bytes()),
            &hex(&artifact.time_domain.to_be_bytes()),
            ProblemEvidence::PersistenceWrongTimeDomain {
                evidence: identity_evidence(
                    "decode",
                    &hex(&context.time_domain().to_be_bytes()),
                    &hex(&artifact.time_domain.to_be_bytes()),
                ),
                marker: PhantomData,
            },
        )));
    }
    let status = match artifact.lifecycle {
        Lifecycle::Awaiting => MachineStatus::AwaitingInitialization,
        Lifecycle::Ready(ready) => MachineStatus::Ready {
            now: Time::from_ticks(ready.time),
        },
    };
    Ok(MachineSnapshot::from_decoded(
        status,
        artifact.revision,
        artifact.fingerprint,
        artifact.execution,
        artifact.observable,
        artifact.snapshot,
        artifact.policy,
        artifact.time_domain,
        artifact.bytes,
    ))
}

impl<D> CompiledNetwork<D> {
    /// Restores one snapshot onto this compiled topology and runtime policy.
    ///
    /// The network is borrowed and is not modified. The snapshot and policy are
    /// taken by value. Success returns one complete machine. Failure returns
    /// no machine.
    pub fn restore(
        &self,
        snapshot: MachineSnapshot<D>,
        policy: RuntimePolicy,
    ) -> Result<Machine<D>, RestoreFailure<D>> {
        let artifact = check_artifact(
            snapshot.artifact_bytes(),
            &DecodePolicy::for_recheck(snapshot.artifact_bytes().len() as u64),
        )
        .map_err(RestoreFailure::Decode)?;
        restore_checked(self, artifact, policy)
    }
}

fn restore_checked<D>(
    compiled: &CompiledNetwork<D>,
    artifact: Artifact,
    policy: RuntimePolicy,
) -> Result<Machine<D>, RestoreFailure<D>> {
    if artifact.time_domain != compiled.time_domain_id() {
        return Err(restore_identity(
            RestoreFailure::WrongTimeDomain,
            "time_domain",
            &hex(&compiled.time_domain_id().to_be_bytes()),
            &hex(&artifact.time_domain.to_be_bytes()),
        ));
    }
    if artifact.network_key != compiled.network_key() {
        return Err(restore_identity(
            RestoreFailure::NetworkIdentityMismatch,
            "network_key",
            &hex(&compiled.network_key().as_u128().to_be_bytes()),
            &hex(&artifact.network_key.as_u128().to_be_bytes()),
        ));
    }
    if artifact.fingerprint != compiled.fingerprint() {
        return Err(restore_identity(
            RestoreFailure::FingerprintMismatch,
            "network_fingerprint",
            &hex(&compiled.fingerprint().as_bytes()),
            &hex(&artifact.fingerprint.as_bytes()),
        ));
    }
    if policy.id() != artifact.policy {
        return Err(restore_identity(
            RestoreFailure::RuntimePolicyMismatch,
            "runtime_policy",
            &hex(&policy.id().as_bytes()),
            &hex(&artifact.policy.as_bytes()),
        ));
    }
    check_standard_expansion(compiled)?;
    let mut installed = install_state(compiled, &artifact)?;
    check_pending(compiled, &artifact, &mut installed)?;
    check_singular_references(&installed)?;
    let episodes = check_episodes(compiled, &artifact)?;
    let provenance = check_provenance(compiled, &artifact, &installed, episodes)?;
    let evaluation = match &artifact.lifecycle {
        Lifecycle::Ready(ready) => Some(check_settled(compiled, &installed, ready)?),
        Lifecycle::Awaiting => None,
    };
    let machine = publish_machine(
        compiled, policy, &artifact, installed, provenance, evaluation,
    )?;
    check_reencoded_provenance(&machine, &artifact)?;
    check_digests(&machine, &artifact)?;
    #[cfg(test)]
    crate::state_digest_reference::assert_machine(&machine);
    Ok(machine)
}

fn persistence_problem<D>(evidence: ProblemEvidence<D>) -> Problem<D> {
    Problem::new(
        SubjectRef::Operation(OperationSubjectRef::Persistence),
        Vec::new(),
        evidence,
    )
}

fn standard_problem<D>(evidence: ProblemEvidence<D>) -> Problem<D> {
    Problem::new(SubjectRef::StandardCatalogue, Vec::new(), evidence)
}

fn identity_evidence(
    stage: &'static str,
    expected: &str,
    actual: &str,
) -> ArtifactIdentityEvidence {
    ArtifactIdentityEvidence {
        artifact_kind: "machine_snapshot".to_owned(),
        stage,
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    }
}

fn identity_problem<D>(
    stage: &'static str,
    expected: &str,
    actual: &str,
    evidence: ProblemEvidence<D>,
) -> Problem<D> {
    let _ = (stage, expected, actual);
    persistence_problem(evidence)
}

fn restore_identity<D>(
    wrap: fn(Problem<D>) -> RestoreFailure<D>,
    stage: &'static str,
    expected: &str,
    actual: &str,
) -> RestoreFailure<D> {
    let evidence = identity_evidence(stage, expected, actual);
    let problem = persistence_problem(match stage {
        "time_domain" => ProblemEvidence::PersistenceWrongTimeDomain {
            evidence,
            marker: PhantomData,
        },
        "network_key" => ProblemEvidence::PersistenceNetworkIdentityMismatch {
            evidence,
            marker: PhantomData,
        },
        "network_fingerprint" => ProblemEvidence::PersistenceFingerprintMismatch {
            evidence,
            marker: PhantomData,
        },
        "topology_revision" => ProblemEvidence::PersistenceTopologyRevisionMismatch {
            evidence,
            marker: PhantomData,
        },
        _ => ProblemEvidence::PersistenceRuntimePolicyMismatch {
            evidence,
            marker: PhantomData,
        },
    });
    wrap(problem)
}

fn canonical_evidence(
    path: &str,
    violation: &'static str,
    encountered: impl Into<String>,
) -> CanonicalEncodingEvidence {
    CanonicalEncodingEvidence {
        artifact_kind: Some("machine_snapshot".to_owned()),
        path: path.to_owned(),
        violation,
        encountered: encountered.into(),
    }
}

fn decode_canonical<D>(
    variant: DecodeCanonical<D>,
    path: &str,
    violation: &'static str,
    encountered: impl Into<String>,
) -> DecodeFailure<D> {
    let evidence = canonical_evidence(path, violation, encountered);
    let problem = persistence_problem(match variant {
        DecodeCanonical::Prefix => ProblemEvidence::PersistenceInvalidPrefix {
            evidence,
            marker: PhantomData,
        },
        DecodeCanonical::Truncated => ProblemEvidence::PersistenceTruncatedArtifact {
            evidence,
            marker: PhantomData,
        },
        DecodeCanonical::Trailing => ProblemEvidence::PersistenceTrailingBytes {
            evidence,
            marker: PhantomData,
        },
        DecodeCanonical::Noncanonical => ProblemEvidence::PersistenceNoncanonicalEncoding {
            evidence,
            marker: PhantomData,
        },
        DecodeCanonical::Malformed(_) => ProblemEvidence::PersistenceMalformedEnvelope {
            evidence,
            marker: PhantomData,
        },
    });
    match variant {
        DecodeCanonical::Prefix => DecodeFailure::InvalidPrefix(problem),
        DecodeCanonical::Truncated => DecodeFailure::TruncatedArtifact(problem),
        DecodeCanonical::Trailing => DecodeFailure::TrailingBytes(problem),
        DecodeCanonical::Noncanonical => DecodeFailure::NoncanonicalEncoding(problem),
        DecodeCanonical::Malformed(_) => DecodeFailure::MalformedEnvelope(problem),
    }
}

enum DecodeCanonical<D> {
    Prefix,
    Truncated,
    Trailing,
    Noncanonical,
    Malformed(PhantomData<D>),
}

impl<D> Copy for DecodeCanonical<D> {}
impl<D> Clone for DecodeCanonical<D> {
    fn clone(&self) -> Self {
        *self
    }
}

fn malformed<D>(path: &str, encountered: impl Into<String>) -> DecodeFailure<D> {
    decode_canonical(
        DecodeCanonical::Malformed(PhantomData),
        path,
        "malformed_envelope",
        encountered,
    )
}

fn noncanonical<D>(
    path: &str,
    violation: &'static str,
    encountered: impl Into<String>,
) -> DecodeFailure<D> {
    decode_canonical(DecodeCanonical::Noncanonical, path, violation, encountered)
}

fn limit_failure<D>(budget: &'static str, limit: u64, consumed: u64) -> DecodeFailure<D> {
    DecodeFailure::DecodeLimitExceeded(persistence_problem(
        ProblemEvidence::PersistenceDecodeLimitExceeded {
            evidence: BudgetEvidence {
                budget,
                limit,
                consumed,
            },
            marker: PhantomData,
        },
    ))
}

fn unsupported_version<D>(
    stage: &'static str,
    component: &str,
    encountered: u64,
) -> DecodeFailure<D> {
    DecodeFailure::UnsupportedVersion(persistence_problem(
        ProblemEvidence::PersistenceUnsupportedVersion {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: "machine_snapshot".to_owned(),
                stage,
                component: component.to_owned(),
                encountered: encountered.to_string(),
                required: SUPPORTED_VERSION.to_string(),
                upgrader_exists: false,
            },
            marker: PhantomData,
        },
    ))
}

fn unknown_field<D>(path: &str, name: &str) -> DecodeFailure<D> {
    DecodeFailure::UnknownSchemaField(persistence_problem(
        ProblemEvidence::PersistenceUnknownSchemaField {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: "machine_snapshot".to_owned(),
                stage: "schema",
                component: format!("{path}.{name}"),
                encountered: name.to_owned(),
                required: "declared field".to_owned(),
                upgrader_exists: false,
            },
            marker: PhantomData,
        },
    ))
}

fn unknown_variant<D>(path: &str, name: &str) -> DecodeFailure<D> {
    DecodeFailure::UnknownSchemaVariant(persistence_problem(
        ProblemEvidence::PersistenceUnknownSchemaVariant {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: "machine_snapshot".to_owned(),
                stage: "schema",
                component: path.to_owned(),
                encountered: name.to_owned(),
                required: "declared variant".to_owned(),
                upgrader_exists: false,
            },
            marker: PhantomData,
        },
    ))
}

fn unknown_kind<D>(kind: &str) -> DecodeFailure<D> {
    DecodeFailure::UnknownArtifactKind(persistence_problem(
        ProblemEvidence::PersistenceUnknownArtifactKind {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: kind.to_owned(),
                stage: "envelope",
                component: "artifact_kind".to_owned(),
                encountered: kind.to_owned(),
                required: "machine_snapshot".to_owned(),
                upgrader_exists: false,
            },
            marker: PhantomData,
        },
    ))
}

fn map_cbor<D>(error: DecodeError) -> DecodeFailure<D> {
    match error {
        DecodeError::Truncated => decode_canonical(
            DecodeCanonical::Truncated,
            "body",
            "truncated_artifact",
            "incomplete CBOR value",
        ),
        DecodeError::Noncanonical {
            violation,
            encountered,
        } => noncanonical("body", violation, encountered),
        DecodeError::Limit {
            budget,
            limit,
            consumed,
        } => limit_failure(budget, limit, consumed),
    }
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

fn recognized_kind(kind: &str) -> bool {
    matches!(
        kind,
        "module_definition"
            | "network_definition"
            | "runtime_policy"
            | "input_snapshot"
            | "input_delta"
            | "network_patch"
            | "transaction_record"
            | "machine_snapshot"
            | "replay_frame"
            | "replay_log"
            | "transaction_result"
            | "migration_report"
            | "checkpoint_bundle"
    )
}

struct Artifact {
    bytes: Vec<u8>,
    payload_bytes: Vec<u8>,
    time_domain: TimeDomainId,
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    execution: ExecutionStateDigest,
    observable: ObservableStateDigest,
    snapshot: SnapshotDigest,
    policy: RuntimePolicyId,
    revision: NetworkRevision,
    next_serial: u64,
    lifecycle: Lifecycle,
    node_state: Vec<StateEntry>,
    temporal_state: Vec<StateEntry>,
    episodes: Vec<EpisodeEntry>,
    provenance: ProvenanceSection,
}

struct LifecycleReady {
    order: u64,
    boundary: Boundary,
    time: u64,
    levels: Vec<(u128, LogicLevel)>,
    baselines: Vec<BaselineEntry>,
    pending: Vec<PendingEntry>,
    settled: Vec<SettledEntry>,
}

enum Lifecycle {
    Awaiting,
    Ready(LifecycleReady),
}

enum Boundary {
    Initialization,
    Checkpoint(&'static str),
}

struct StateEntry {
    owner_bytes: Vec<u8>,
    owner: StableName,
    kind: String,
    schema: String,
    value: ParsedState,
}

struct StableName {
    instances: Vec<u128>,
    node: u128,
}

#[derive(Clone, Copy)]
enum ParsedState {
    Level(LogicLevel),
    Edge(EdgeObservation),
    Remembered {
        remembered: LogicLevel,
        output: LogicLevel,
    },
    Periodic {
        phase: Option<(u64, u64)>,
        settled: Option<u64>,
        anchor: Option<u64>,
        previous: LogicLevel,
    },
    Temporal(Option<u64>),
}

struct BaselineEntry {
    key: u128,
    level: LogicLevel,
    established: bool,
    cause: [u8; 32],
}

struct PendingEntry {
    stimulus: (u64, u64),
    key: u64,
    owner_bytes: Vec<u8>,
    owner: StableName,
    kind_name: &'static str,
    kind: PendingKind,
    origin: u64,
    deadline: u64,
    revision: u64,
    cause: [u8; 32],
}

#[derive(Clone, Copy)]
enum PendingKind {
    Pulse { count: u64 },
    Transport { origin: u64, target: LogicLevel },
    Inertial { origin: u64, target: LogicLevel },
    Periodic { anchor: u64, ordinal: u64 },
}

struct SettledEntry {
    subject_bytes: Vec<u8>,
    level: LogicLevel,
}

struct EpisodeEntry {
    began_order: u64,
    changed_order: u64,
    identity: [u8; 32],
    code: String,
    discriminator: u64,
    began_at: u64,
    last_material_change: u64,
    revision: u64,
    owner: StableName,
    cause: [u8; 32],
    evidence: EpisodeEvidence,
}

struct EpisodeEvidence {
    policy: ConflictPolicy,
    previous: LogicLevel,
    controls: ConflictControls,
    revision: u64,
}

struct ProvenanceSection {
    episodes: Vec<[u8; 32]>,
    external_inputs: Vec<[u8; 32]>,
    output_baselines: Vec<[u8; 32]>,
    pending_events: Vec<[u8; 32]>,
    state: Vec<[u8; 32]>,
    records: Vec<ParsedRecord>,
}

struct ParsedRecord {
    bytes: Vec<u8>,
    digest: [u8; 32],
    kind: RecordKind,
    predecessors: Vec<Predecessor>,
    revision: Option<u64>,
    subject: Option<ParsedSubject>,
    time: Option<u64>,
    order: Option<u64>,
    stimulus_time: Option<u64>,
    inapplicable: bool,
}

enum RecordKind {
    TopologyChange {
        base: [u8; 32],
        target: [u8; 32],
    },
    Migration {
        rule: String,
    },
    Checkpoint {
        fact: Vec<u8>,
    },
    Initialization,
    Ready,
    ExternalObservation {
        value: LogicLevel,
    },
    ExternalPulse {
        count: u64,
    },
    PendingPulse {
        count: u64,
    },
    PendingTransport {
        target: LogicLevel,
    },
    PendingInertial {
        target: LogicLevel,
    },
    PendingPeriodic {
        anchor: u64,
        ordinal: u64,
        first_emission: FirstEmissionPolicy,
        reenable_phase: ReenablePhasePolicy,
    },
    Derived,
    PulseDerived {
        result: u64,
    },
    PulseControlled {
        result: LogicLevel,
    },
}

struct Predecessor {
    role: PredecessorRole,
    digest: [u8; 32],
    contribution: Option<Contribution>,
}

enum PredecessorRole {
    Supporter,
    Contribution,
    Other(String),
}

struct Contribution {
    count: u64,
    port: PortName,
}

enum PortName {
    Top(u128),
    Module { instances: Vec<u128>, port: u128 },
}

enum ParsedSubject {
    ExternalInput(u128),
    ExternalPulseInput(u128),
    ExternalOutput(u128),
    ExternalPulseOutput(u128),
    Node(StableName),
}

struct Counters {
    policy: DecodePolicy,
    nodes: u64,
    instances: BTreeSet<u128>,
    edges: u64,
}

fn check_artifact<D>(bytes: &[u8], policy: &DecodePolicy) -> Result<Artifact, DecodeFailure<D>> {
    // Ports, connections, and replay frames stay on the policy and do not
    // bound machine-snapshot structure.
    let _ = (policy.ports(), policy.connections(), policy.replay_frames());
    // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "hostile-and-bounded-decode"
    // The total byte budget counts the whole artifact, including the prefix.
    if bytes.len() as u64 > policy.total_bytes() {
        return Err(limit_failure(
            "total_bytes",
            policy.total_bytes(),
            bytes.len() as u64,
        ));
    }
    if bytes.len() < ARTIFACT_PREFIX.len() {
        return Err(if ARTIFACT_PREFIX.starts_with(bytes) {
            decode_canonical(
                DecodeCanonical::Truncated,
                "prefix",
                "truncated_artifact",
                bytes.len().to_string(),
            )
        } else {
            decode_canonical(
                DecodeCanonical::Prefix,
                "prefix",
                "invalid_prefix",
                bytes.len().to_string(),
            )
        });
    }
    if bytes[..ARTIFACT_PREFIX.len()] != ARTIFACT_PREFIX {
        return Err(decode_canonical(
            DecodeCanonical::Prefix,
            "prefix",
            "invalid_prefix",
            hex(&bytes[..ARTIFACT_PREFIX.len()]),
        ));
    }
    if bytes.len() == ARTIFACT_PREFIX.len() {
        return Err(decode_canonical(
            DecodeCanonical::Truncated,
            "body",
            "truncated_artifact",
            "prefix only",
        ));
    }
    let body = &bytes[ARTIFACT_PREFIX.len()..];
    let (value, consumed) = cbor_decode::parse(
        body,
        Limits {
            total_bytes: policy.total_bytes(),
            nesting: policy.nesting(),
            text_bytes: policy.text_bytes(),
            byte_string_bytes: policy.byte_string_bytes(),
            collection_items: policy.collection_items(),
        },
    )
    .map_err(map_cbor)?;
    if consumed != body.len() {
        return Err(decode_canonical(
            DecodeCanonical::Trailing,
            "body",
            "trailing_bytes",
            (body.len() - consumed).to_string(),
        ));
    }
    if cbor_decode::encode(&value) != body {
        return Err(noncanonical("body", "re_encode_mismatch", "body"));
    }
    interpret_artifact(bytes.to_vec(), value, *policy)
}

fn interpret_artifact<D>(
    bytes: Vec<u8>,
    value: Value,
    policy: DecodePolicy,
) -> Result<Artifact, DecodeFailure<D>> {
    let mut items = expect_array(value, "artifact")?;
    if items.len() != 2 {
        return Err(malformed("artifact", "wrapper length"));
    }
    let name = expect_text(items.remove(0), "artifact.kind")?;
    if name != "mossignal_artifact" {
        return Err(malformed("artifact", name));
    }
    let mut fields = into_fields(items.remove(0), "envelope")?;
    reject_unknown_fields(&fields, ENVELOPE_FIELDS, "envelope")?;
    // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "canonical-framing"
    // Kind is identified before integrity, so a same-length kind swap fails as
    // an envelope or kind error. The digest still covers artifact_kind.
    let kind = expect_text(
        take_required_owned(&fields, "artifact_kind", "envelope")?,
        "envelope.artifact_kind",
    )?;
    if kind != "machine_snapshot" {
        return Err(if recognized_kind(&kind) {
            malformed("envelope.artifact_kind", kind)
        } else {
            unknown_kind(&kind)
        });
    }
    let integrity = expect_digest(
        take_required(&mut fields, "integrity_digest", "envelope")?,
        "envelope.integrity_digest",
    )?;
    let bare_pairs: Vec<(&str, &Value)> = fields
        .iter()
        .map(|(name, value)| (name.as_str(), value))
        .collect();
    let bare = cbor_decode::encode_named_pairs(&bare_pairs);
    let actual = *blake3::hash(&domain_separated(SNAPSHOT_DIGEST_DOMAIN, 2, &bare)).as_bytes();
    if actual != integrity {
        return Err(DecodeFailure::IntegrityDigestMismatch(persistence_problem(
            ProblemEvidence::PersistenceIntegrityDigestMismatch {
                evidence: DigestMismatchEvidence {
                    kind: "snapshot",
                    expected: hex(&integrity),
                    actual: hex(&actual),
                    context: "envelope".to_owned(),
                },
                marker: PhantomData,
            },
        )));
    }
    for (name, value) in &fields {
        if name.ends_with("_version") {
            let version = expect_uint(value_ref(value), &format!("envelope.{name}"))?;
            if version != SUPPORTED_VERSION {
                return Err(unsupported_version("envelope", name, version));
            }
        }
    }
    // SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "envelope-and-kind"
    // Each envelope version field is present exactly once. Absence is not an
    // unsupported version.
    for name in ENVELOPE_FIELDS {
        if name.ends_with("_version") && !fields.contains_key(*name) {
            return Err(malformed("envelope", *name));
        }
    }
    let time_domain = expect_time_domain(
        take_required(&mut fields, "time_domain_id", "envelope")?,
        "envelope.time_domain_id",
    )?;
    let payload = take_required(&mut fields, "payload", "envelope")?;
    let payload_bytes = cbor_decode::encode(&payload);
    let mut counters = Counters {
        policy,
        nodes: 0,
        instances: BTreeSet::new(),
        edges: 0,
    };
    parse_payload(
        bytes,
        payload_bytes,
        payload,
        time_domain,
        SnapshotDigest::from_digest(integrity),
        &mut counters,
    )
}

fn value_ref(value: &Value) -> &Value {
    value
}

fn parse_payload<D>(
    bytes: Vec<u8>,
    payload_bytes: Vec<u8>,
    payload: Value,
    time_domain: TimeDomainId,
    snapshot: SnapshotDigest,
    counters: &mut Counters,
) -> Result<Artifact, DecodeFailure<D>> {
    let mut fields = into_fields(payload, "payload")?;
    reject_unknown_fields(&fields, PAYLOAD_FIELDS, "payload")?;
    let versions = into_fields(
        take_required(&mut fields, "semantic_versions", "payload")?,
        "payload.semantic_versions",
    )?;
    reject_unknown_fields(&versions, SEMANTIC_FIELDS, "payload.semantic_versions")?;
    for name in SEMANTIC_FIELDS {
        let version = expect_uint(
            &take_required_owned(&versions, name, "payload.semantic_versions")?,
            &format!("payload.semantic_versions.{name}"),
        )?;
        if version != SUPPORTED_VERSION {
            return Err(unsupported_version("payload", name, version));
        }
    }
    let payload_domain = expect_time_domain(
        take_required(&mut fields, "time_domain_id", "payload")?,
        "payload.time_domain_id",
    )?;
    if payload_domain != time_domain {
        return Err(DecodeFailure::WrongTimeDomain(persistence_problem(
            ProblemEvidence::PersistenceWrongTimeDomain {
                evidence: identity_evidence(
                    "payload",
                    &hex(&time_domain.to_be_bytes()),
                    &hex(&payload_domain.to_be_bytes()),
                ),
                marker: PhantomData,
            },
        )));
    }
    if let Some(history) = take_optional(&mut fields, "optional_history") {
        let size = cbor_decode::encode(&history).len() as u64;
        if size > counters.policy.optional_history_bytes() {
            return Err(limit_failure(
                "optional_history_bytes",
                counters.policy.optional_history_bytes(),
                size,
            ));
        }
    }
    if let Some(metadata) = take_optional(&mut fields, "persistence_metadata") {
        let mut meta = into_fields(metadata, "payload.persistence_metadata")?;
        reject_unknown_fields(&meta, &["label"], "payload.persistence_metadata")?;
        let _ = expect_text(
            take_required(&mut meta, "label", "payload.persistence_metadata")?,
            "payload.persistence_metadata.label",
        )?;
    }
    let network_key = expect_key(
        take_required(&mut fields, "network_key", "payload")?,
        "payload.network_key",
    )?;
    let fingerprint = NetworkFingerprint::from_digest(expect_digest(
        take_required(&mut fields, "network_fingerprint", "payload")?,
        "payload.network_fingerprint",
    )?);
    let execution = ExecutionStateDigest::from_digest(expect_digest(
        take_required(&mut fields, "execution_state_digest", "payload")?,
        "payload.execution_state_digest",
    )?);
    let observable = ObservableStateDigest::from_digest(expect_digest(
        take_required(&mut fields, "observable_state_digest", "payload")?,
        "payload.observable_state_digest",
    )?);
    let policy_id = RuntimePolicyId::from_digest(expect_digest(
        take_required(&mut fields, "runtime_policy_id", "payload")?,
        "payload.runtime_policy_id",
    )?);
    let revision = NetworkRevision::from_value(expect_uint(
        &take_required(&mut fields, "topology_revision", "payload")?,
        "payload.topology_revision",
    )?);
    let next_serial = expect_uint(
        &take_required(&mut fields, "next_pending_event_serial", "payload")?,
        "payload.next_pending_event_serial",
    )?;
    let node_state = parse_state_table(
        take_required(&mut fields, "node_state_table", "payload")?,
        "payload.node_state_table",
        counters,
    )?;
    let temporal_state = parse_state_table(
        take_required(&mut fields, "temporal_state_table", "payload")?,
        "payload.temporal_state_table",
        counters,
    )?;
    let episodes = parse_episodes(
        take_required(&mut fields, "active_diagnostic_episodes", "payload")?,
        counters,
    )?;
    let lifecycle = parse_lifecycle(
        take_required(&mut fields, "lifecycle", "payload")?,
        counters,
    )?;
    let provenance = parse_provenance(
        take_required(&mut fields, "provenance", "payload")?,
        counters,
    )?;
    Ok(Artifact {
        bytes,
        payload_bytes,
        time_domain,
        network_key: NetworkKey::from_u128(network_key),
        fingerprint,
        execution,
        observable,
        snapshot,
        policy: policy_id,
        revision,
        next_serial,
        lifecycle,
        node_state,
        temporal_state,
        episodes,
        provenance,
    })
}

fn expect_array<D>(value: Value, path: &str) -> Result<Vec<Value>, DecodeFailure<D>> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(malformed(path, "array")),
    }
}

fn expect_text<D>(value: Value, path: &str) -> Result<String, DecodeFailure<D>> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(malformed(path, "text")),
    }
}

fn expect_uint<D>(value: &Value, path: &str) -> Result<u64, DecodeFailure<D>> {
    match value {
        Value::Uint(value) => Ok(*value),
        _ => Err(malformed(path, "integer")),
    }
}

fn expect_bool<D>(value: Value, path: &str) -> Result<bool, DecodeFailure<D>> {
    match value {
        Value::Bool(value) => Ok(value),
        _ => Err(malformed(path, "boolean")),
    }
}

fn expect_digest<D>(value: Value, path: &str) -> Result<[u8; 32], DecodeFailure<D>> {
    fixed_bytes(value, path)
}

fn expect_key<D>(value: Value, path: &str) -> Result<u128, DecodeFailure<D>> {
    let bytes: [u8; 16] = fixed_bytes(value, path)?;
    Ok(u128::from_be_bytes(bytes))
}

fn expect_time_domain<D>(value: Value, path: &str) -> Result<TimeDomainId, DecodeFailure<D>> {
    Ok(TimeDomainId::from_u128(expect_key(value, path)?))
}

fn fixed_bytes<D, const N: usize>(value: Value, path: &str) -> Result<[u8; N], DecodeFailure<D>> {
    match value {
        Value::Bytes(bytes) if bytes.len() == N => {
            let mut raw = [0; N];
            raw.copy_from_slice(&bytes);
            Ok(raw)
        }
        Value::Bytes(bytes) => Err(noncanonical(
            path,
            "wrong_fixed_length",
            bytes.len().to_string(),
        )),
        _ => Err(malformed(path, "byte string")),
    }
}

fn into_fields<D>(value: Value, path: &str) -> Result<BTreeMap<String, Value>, DecodeFailure<D>> {
    let items = expect_array(value, path)?;
    let mut fields = BTreeMap::new();
    let mut previous: Option<String> = None;
    for item in items {
        let mut pair = expect_array(item, path)?;
        if pair.len() != 2 {
            return Err(malformed(path, "field length"));
        }
        let name = expect_text(pair.remove(0), path)?;
        if let Some(previous) = &previous {
            if name.as_str() < previous.as_str() {
                return Err(noncanonical(path, "unsorted_field", name));
            }
            if name == *previous {
                return Err(noncanonical(path, "duplicate_field", name));
            }
        }
        previous = Some(name.clone());
        fields.insert(name, pair.remove(0));
    }
    Ok(fields)
}

fn reject_unknown_fields<D>(
    fields: &BTreeMap<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), DecodeFailure<D>> {
    for name in fields.keys() {
        if !allowed.contains(&name.as_str()) {
            return Err(unknown_field(path, name));
        }
    }
    Ok(())
}

fn take_required<D>(
    fields: &mut BTreeMap<String, Value>,
    name: &str,
    path: &str,
) -> Result<Value, DecodeFailure<D>> {
    fields
        .remove(name)
        .ok_or_else(|| malformed(path, format!("missing {name}")))
}

fn take_required_owned<D>(
    fields: &BTreeMap<String, Value>,
    name: &str,
    path: &str,
) -> Result<Value, DecodeFailure<D>> {
    fields
        .get(name)
        .cloned()
        .ok_or_else(|| malformed(path, format!("missing {name}")))
}

fn take_optional(fields: &mut BTreeMap<String, Value>, name: &str) -> Option<Value> {
    fields.remove(name)
}

fn expect_variant<D>(value: Value, path: &str) -> Result<(String, Value), DecodeFailure<D>> {
    let mut items = expect_array(value, path)?;
    if items.len() != 2 {
        return Err(malformed(path, "variant length"));
    }
    let name = expect_text(items.remove(0), path)?;
    Ok((name, items.remove(0)))
}

fn require_null<D>(value: Value, path: &str) -> Result<(), DecodeFailure<D>> {
    match value {
        Value::Null => Ok(()),
        _ => Err(malformed(path, "variant body")),
    }
}

fn named_variant<D>(
    value: Value,
    allowed: &[&str],
    path: &str,
) -> Result<(String, Value), DecodeFailure<D>> {
    let (name, body) = expect_variant(value, path)?;
    if !allowed.contains(&name.as_str()) {
        return Err(unknown_variant(path, &name));
    }
    Ok((name, body))
}

fn charge<D>(consumed: &mut u64, limit: u64, budget: &'static str) -> Result<(), DecodeFailure<D>> {
    *consumed = consumed.saturating_add(1);
    if *consumed > limit {
        return Err(limit_failure(budget, limit, *consumed));
    }
    Ok(())
}

fn charge_path<D>(counters: &mut Counters, instances: &[u128]) -> Result<(), DecodeFailure<D>> {
    let limit = counters.policy.module_instances();
    let length = u64::try_from(instances.len()).unwrap_or(u64::MAX);
    if length > limit {
        return Err(limit_failure("module_instances", limit, length));
    }
    for instance in instances {
        counters.instances.insert(*instance);
    }
    let distinct = u64::try_from(counters.instances.len()).unwrap_or(u64::MAX);
    if distinct > limit {
        return Err(limit_failure("module_instances", limit, distinct));
    }
    Ok(())
}

fn strict_increase<D>(
    previous: &mut Option<Vec<u8>>,
    next: &[u8],
    path: &str,
) -> Result<(), DecodeFailure<D>> {
    if previous
        .as_ref()
        .is_some_and(|previous| next <= previous.as_slice())
    {
        return Err(noncanonical(path, "unsorted_set_or_map", "order"));
    }
    *previous = Some(next.to_vec());
    Ok(())
}

fn nondecreasing<D>(
    previous: &mut Option<Vec<u8>>,
    next: &[u8],
    path: &str,
) -> Result<(), DecodeFailure<D>> {
    if previous
        .as_ref()
        .is_some_and(|previous| next < previous.as_slice())
    {
        return Err(noncanonical(path, "unsorted_set_or_map", "order"));
    }
    *previous = Some(next.to_vec());
    Ok(())
}

fn order_key(parts: &[&[u8]]) -> Vec<u8> {
    let mut key = Vec::new();
    for part in parts {
        key.extend_from_slice(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
        key.extend_from_slice(part);
    }
    key
}

fn parse_state_table<D>(
    value: Value,
    path: &str,
    counters: &mut Counters,
) -> Result<Vec<StateEntry>, DecodeFailure<D>> {
    let items = expect_array(value, path)?;
    let mut entries = Vec::with_capacity(items.len());
    let mut previous = None;
    for item in items {
        charge(&mut counters.nodes, counters.policy.nodes(), "nodes")?;
        let entry = parse_state_entry(item, path, counters)?;
        let schema = entry.schema.as_bytes().to_vec();
        nondecreasing(
            &mut previous,
            &order_key(&[entry.owner_bytes.as_slice(), schema.as_slice()]),
            path,
        )?;
        entries.push(entry);
    }
    Ok(entries)
}

fn parse_state_entry<D>(
    value: Value,
    path: &str,
    counters: &mut Counters,
) -> Result<StateEntry, DecodeFailure<D>> {
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(&fields, &["node_kind", "owner", "schema", "value"], path)?;
    let (kind, kind_body) = named_variant(
        take_required(&mut fields, "node_kind", path)?,
        NODE_KINDS,
        path,
    )?;
    require_null(kind_body, path)?;
    let owner_value = take_required(&mut fields, "owner", path)?;
    let owner_bytes = cbor_decode::encode(&owner_value);
    let owner = parse_stable_name(owner_value, counters, path)?;
    let (schema, schema_body) = named_variant(
        take_required(&mut fields, "schema", path)?,
        STATE_SCHEMAS,
        path,
    )?;
    require_null(schema_body, path)?;
    let value = parse_state_value(take_required(&mut fields, "value", path)?, path)?;
    Ok(StateEntry {
        owner_bytes,
        owner,
        kind,
        schema,
        value,
    })
}

fn parse_state_value<D>(value: Value, path: &str) -> Result<ParsedState, DecodeFailure<D>> {
    if matches!(value, Value::Null) {
        return Ok(ParsedState::Temporal(None));
    }
    if let Value::Array(items) = &value {
        if items.iter().all(|item| matches!(item, Value::Array(_))) {
            return parse_state_record(value, path);
        }
    }
    let (name, body) = expect_variant(value, path)?;
    match name.as_str() {
        "low" => {
            require_null(body, path)?;
            Ok(ParsedState::Level(LogicLevel::Low))
        }
        "high" => {
            require_null(body, path)?;
            Ok(ParsedState::Level(LogicLevel::High))
        }
        "unestablished" => {
            require_null(body, path)?;
            Ok(ParsedState::Edge(EdgeObservation::Unestablished))
        }
        "established" => Ok(ParsedState::Edge(EdgeObservation::Established(
            expect_level(body, path)?,
        ))),
        _ => Err(unknown_variant(path, &name)),
    }
}

fn parse_state_record<D>(value: Value, path: &str) -> Result<ParsedState, DecodeFailure<D>> {
    let mut fields = into_fields(value, path)?;
    if fields.contains_key("event") {
        reject_unknown_fields(&fields, &["event"], path)?;
        let serial = expect_uint(&take_required(&mut fields, "event", path)?, path)?;
        return Ok(ParsedState::Temporal(Some(serial)));
    }
    if fields.contains_key("output") || fields.contains_key("remembered_input") {
        reject_unknown_fields(&fields, &["output", "remembered_input"], path)?;
        let output = expect_level(take_required(&mut fields, "output", path)?, path)?;
        let remembered = expect_level(take_required(&mut fields, "remembered_input", path)?, path)?;
        return Ok(ParsedState::Remembered { remembered, output });
    }
    if fields.contains_key("previous_enable") || fields.contains_key("anchor") {
        reject_unknown_fields(
            &fields,
            &[
                "anchor",
                "previous_enable",
                "phase_time",
                "phase_order",
                "settled_boundary",
            ],
            path,
        )?;
        let anchor = match take_optional(&mut fields, "anchor") {
            Some(value) => Some(expect_uint(&value, path)?),
            None => None,
        };
        let previous = expect_level(take_required(&mut fields, "previous_enable", path)?, path)?;
        let phase_time = take_optional(&mut fields, "phase_time")
            .map(|v| expect_uint(&v, path))
            .transpose()?;
        let phase_order = take_optional(&mut fields, "phase_order")
            .map(|v| expect_uint(&v, path))
            .transpose()?;
        let settled = take_optional(&mut fields, "settled_boundary")
            .map(|v| expect_uint(&v, path))
            .transpose()?;
        let phase = phase_time.zip(phase_order);
        if anchor.is_some() != phase.is_some()
            || phase_time.is_some() != phase_order.is_some()
            || (anchor.is_none() && settled.is_some())
        {
            return Err(malformed(path, "periodic phase"));
        }
        return Ok(ParsedState::Periodic {
            anchor,
            previous,
            phase,
            settled,
        });
    }
    if let Some(name) = fields.keys().next() {
        return Err(unknown_field(path, name));
    }
    Err(malformed(path, "state value"))
}

fn expect_level<D>(value: Value, path: &str) -> Result<LogicLevel, DecodeFailure<D>> {
    match parse_state_value(value, path)? {
        ParsedState::Level(level) => Ok(level),
        _ => Err(malformed(path, "logic level")),
    }
}

fn parse_stable_name<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<StableName, DecodeFailure<D>> {
    match parse_subject(value, counters, path)? {
        ParsedSubject::Node(name) => Ok(name),
        _ => Err(malformed(path, "node owner")),
    }
}

fn parse_subject<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<ParsedSubject, DecodeFailure<D>> {
    let (name, body) = expect_variant(value, path)?;
    match name.as_str() {
        "node" => Ok(ParsedSubject::Node(StableName {
            instances: Vec::new(),
            node: expect_key(body, path)?,
        })),
        "external_input" => Ok(ParsedSubject::ExternalInput(expect_key(body, path)?)),
        "external_pulse_input" => Ok(ParsedSubject::ExternalPulseInput(expect_key(body, path)?)),
        "external_output" => Ok(ParsedSubject::ExternalOutput(expect_key(body, path)?)),
        "external_pulse_output" => Ok(ParsedSubject::ExternalPulseOutput(expect_key(body, path)?)),
        "module_node" => {
            let name = parse_module_node(body, counters, path)?;
            Ok(ParsedSubject::Node(name))
        }
        _ => Err(unknown_variant(path, &name)),
    }
}

fn parse_module_node<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<StableName, DecodeFailure<D>> {
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(&fields, &["instances", "node"], path)?;
    let instances = parse_instance_path(
        take_required(&mut fields, "instances", path)?,
        counters,
        path,
    )?;
    if instances.is_empty() {
        return Err(malformed(path, "empty instance path"));
    }
    let node = expect_key(take_required(&mut fields, "node", path)?, path)?;
    Ok(StableName { instances, node })
}

fn parse_instance_path<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<Vec<u128>, DecodeFailure<D>> {
    let items = expect_array(value, path)?;
    let mut instances = Vec::with_capacity(items.len());
    for item in items {
        instances.push(expect_key(item, path)?);
    }
    charge_path(counters, &instances)?;
    Ok(instances)
}

fn parse_episodes<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<Vec<EpisodeEntry>, DecodeFailure<D>> {
    let items = expect_array(value, "payload.active_diagnostic_episodes")?;
    let mut episodes = Vec::with_capacity(items.len());
    let mut previous = None;
    let mut consumed = 0;
    for item in items {
        charge(
            &mut consumed,
            counters.policy.diagnostic_records(),
            "diagnostic_records",
        )?;
        let episode = parse_episode(item, counters)?;
        strict_increase(
            &mut previous,
            &episode.identity,
            "payload.active_diagnostic_episodes",
        )?;
        episodes.push(episode);
    }
    Ok(episodes)
}

fn parse_episode<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<EpisodeEntry, DecodeFailure<D>> {
    let path = "payload.active_diagnostic_episodes";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &[
            "began_at",
            "began_order",
            "last_material_order",
            "cause",
            "code",
            "discriminator",
            "evidence",
            "identity",
            "last_material_change",
            "owner",
            "revision",
        ],
        path,
    )?;
    let began_at = expect_uint(&take_required(&mut fields, "began_at", path)?, path)?;
    let cause = expect_digest(take_required(&mut fields, "cause", path)?, path)?;
    let code = expect_text(take_required(&mut fields, "code", path)?, path)?;
    let discriminator = expect_uint(&take_required(&mut fields, "discriminator", path)?, path)?;
    let evidence = parse_episode_evidence(take_required(&mut fields, "evidence", path)?)?;
    let identity = expect_digest(take_required(&mut fields, "identity", path)?, path)?;
    let last_material_change = expect_uint(
        &take_required(&mut fields, "last_material_change", path)?,
        path,
    )?;
    let owner = parse_stable_name(take_required(&mut fields, "owner", path)?, counters, path)?;
    let revision = expect_uint(&take_required(&mut fields, "revision", path)?, path)?;
    let began_order = expect_uint(&take_required(&mut fields, "began_order", path)?, path)?;
    let changed_order = expect_uint(
        &take_required(&mut fields, "last_material_order", path)?,
        path,
    )?;
    Ok(EpisodeEntry {
        began_order,
        changed_order,
        identity,
        code,
        discriminator,
        began_at,
        last_material_change,
        revision,
        owner,
        cause,
        evidence,
    })
}

fn parse_episode_evidence<D>(value: Value) -> Result<EpisodeEvidence, DecodeFailure<D>> {
    let path = "payload.active_diagnostic_episodes.evidence";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &["controls", "policy", "previous", "revision"],
        path,
    )?;
    let controls = parse_controls(take_required(&mut fields, "controls", path)?)?;
    let policy = parse_conflict_policy(take_required(&mut fields, "policy", path)?)?;
    let previous = expect_level(take_required(&mut fields, "previous", path)?, path)?;
    let revision = expect_uint(&take_required(&mut fields, "revision", path)?, path)?;
    Ok(EpisodeEvidence {
        policy,
        previous,
        controls,
        revision,
    })
}

fn parse_conflict_policy<D>(value: Value) -> Result<ConflictPolicy, DecodeFailure<D>> {
    let path = "payload.active_diagnostic_episodes.evidence.policy";
    let (name, body) = named_variant(
        value,
        &[
            "reject_transaction",
            "reset_dominant",
            "retain_and_diagnose",
            "set_dominant",
        ],
        path,
    )?;
    require_null(body, path)?;
    Ok(match name.as_str() {
        "set_dominant" => ConflictPolicy::SetDominant,
        "reset_dominant" => ConflictPolicy::ResetDominant,
        "retain_and_diagnose" => ConflictPolicy::RetainAndDiagnose,
        _ => ConflictPolicy::RejectTransaction,
    })
}

fn parse_controls<D>(value: Value) -> Result<ConflictControls, DecodeFailure<D>> {
    let path = "payload.active_diagnostic_episodes.evidence.controls";
    let (name, body) = named_variant(value, &["level", "pulse"], path)?;
    let mut fields = into_fields(body, path)?;
    reject_unknown_fields(&fields, &["reset", "set"], path)?;
    match name.as_str() {
        "level" => Ok(ConflictControls::Level {
            reset: expect_level(take_required(&mut fields, "reset", path)?, path)?,
            set: expect_level(take_required(&mut fields, "set", path)?, path)?,
        }),
        _ => Ok(ConflictControls::Pulse {
            reset: PulseCount::new(expect_uint(
                &take_required(&mut fields, "reset", path)?,
                path,
            )?),
            set: PulseCount::new(expect_uint(
                &take_required(&mut fields, "set", path)?,
                path,
            )?),
        }),
    }
}

fn parse_lifecycle<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<Lifecycle, DecodeFailure<D>> {
    let path = "payload.lifecycle";
    let (name, body) = expect_variant(value, path)?;
    match name.as_str() {
        "awaiting_initialization" => {
            require_null(body, path)?;
            Ok(Lifecycle::Awaiting)
        }
        "ready" => Ok(Lifecycle::Ready(parse_ready(body, counters)?)),
        _ => Err(unknown_variant(path, &name)),
    }
}

fn parse_ready<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<LifecycleReady, DecodeFailure<D>> {
    let path = "payload.lifecycle";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &[
            "explanation_boundary",
            "external_levels",
            "output_baselines",
            "pending_events",
            "settled_levels",
            "time",
            "reaction_order",
        ],
        path,
    )?;
    let boundary = parse_boundary(take_required(&mut fields, "explanation_boundary", path)?)?;
    let levels = parse_external_levels(take_required(&mut fields, "external_levels", path)?)?;
    let baselines = parse_baselines(take_required(&mut fields, "output_baselines", path)?)?;
    let pending = parse_pending_events(
        take_required(&mut fields, "pending_events", path)?,
        counters,
    )?;
    let settled = parse_settled(take_required(&mut fields, "settled_levels", path)?)?;
    let time = expect_uint(&take_required(&mut fields, "time", path)?, path)?;
    let order = expect_uint(&take_required(&mut fields, "reaction_order", path)?, path)?;
    Ok(LifecycleReady {
        order,
        boundary,
        time,
        levels,
        baselines,
        pending,
        settled,
    })
}

fn parse_boundary<D>(value: Value) -> Result<Boundary, DecodeFailure<D>> {
    let path = "payload.lifecycle.explanation_boundary";
    let (name, body) = expect_variant(value, path)?;
    if name == "complete_from_initialization" {
        require_null(body, path)?;
        return Ok(Boundary::Initialization);
    }
    if CHECKPOINTS.contains(&name.as_str()) {
        return Ok(Boundary::Checkpoint(checkpoint_name(&name)));
    }
    Err(unknown_variant(path, &name))
}

fn checkpoint_name(name: &str) -> &'static str {
    CHECKPOINTS
        .iter()
        .copied()
        .find(|candidate| *candidate == name)
        .unwrap_or("checkpoint")
}

fn parse_external_levels<D>(value: Value) -> Result<Vec<(u128, LogicLevel)>, DecodeFailure<D>> {
    let path = "payload.lifecycle.external_levels";
    let items = expect_array(value, path)?;
    let mut levels = Vec::with_capacity(items.len());
    let mut previous = None;
    for item in items {
        let mut pair = expect_array(item, path)?;
        if pair.len() != 2 {
            return Err(malformed(path, "level length"));
        }
        let key = expect_key(pair.remove(0), path)?;
        strict_increase(&mut previous, &key.to_be_bytes(), path)?;
        levels.push((key, expect_level(pair.remove(0), path)?));
    }
    Ok(levels)
}

fn parse_baselines<D>(value: Value) -> Result<Vec<BaselineEntry>, DecodeFailure<D>> {
    let path = "payload.lifecycle.output_baselines";
    let items = expect_array(value, path)?;
    let mut baselines = Vec::with_capacity(items.len());
    let mut previous = None;
    for item in items {
        let mut fields = into_fields(item, path)?;
        reject_unknown_fields(&fields, &["cause", "established", "key", "level"], path)?;
        let cause = expect_digest(take_required(&mut fields, "cause", path)?, path)?;
        let established = expect_bool(take_required(&mut fields, "established", path)?, path)?;
        let key = expect_key(take_required(&mut fields, "key", path)?, path)?;
        strict_increase(&mut previous, &key.to_be_bytes(), path)?;
        let level = expect_level(take_required(&mut fields, "level", path)?, path)?;
        baselines.push(BaselineEntry {
            key,
            level,
            established,
            cause,
        });
    }
    Ok(baselines)
}

fn parse_settled<D>(value: Value) -> Result<Vec<SettledEntry>, DecodeFailure<D>> {
    let path = "payload.lifecycle.settled_levels";
    let items = expect_array(value, path)?;
    let mut settled = Vec::with_capacity(items.len());
    let mut previous = None;
    for item in items {
        let mut fields = into_fields(item, path)?;
        reject_unknown_fields(&fields, &["level", "subject"], path)?;
        let level = expect_level(take_required(&mut fields, "level", path)?, path)?;
        let subject = take_required(&mut fields, "subject", path)?;
        let subject_bytes = cbor_decode::encode(&subject);
        strict_increase(&mut previous, &subject_bytes, path)?;
        settled.push(SettledEntry {
            subject_bytes,
            level,
        });
    }
    Ok(settled)
}

fn parse_pending_events<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<Vec<PendingEntry>, DecodeFailure<D>> {
    let path = "payload.lifecycle.pending_events";
    let items = expect_array(value, path)?;
    let mut events = Vec::with_capacity(items.len());
    let mut previous = None;
    let mut consumed = 0;
    for item in items {
        charge(
            &mut consumed,
            counters.policy.pending_events(),
            "pending_events",
        )?;
        let event = parse_pending_event(item, counters)?;
        let deadline = event.deadline.to_be_bytes();
        let serial = event.key.to_be_bytes();
        let owner = event.owner_bytes.clone();
        let kind = event.kind_name.as_bytes().to_vec();
        nondecreasing(
            &mut previous,
            &order_key(&[
                deadline.as_slice(),
                owner.as_slice(),
                kind.as_slice(),
                serial.as_slice(),
            ]),
            path,
        )?;
        events.push(event);
    }
    Ok(events)
}

fn parse_pending_event<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<PendingEntry, DecodeFailure<D>> {
    let path = "payload.lifecycle.pending_events";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &[
            "cause",
            "deadline",
            "key",
            "kind",
            "origin_revision",
            "origin_time",
            "stimulus_time",
            "stimulus_order",
            "owner",
        ],
        path,
    )?;
    let cause = expect_digest(take_required(&mut fields, "cause", path)?, path)?;
    let deadline = expect_uint(&take_required(&mut fields, "deadline", path)?, path)?;
    let key = expect_uint(&take_required(&mut fields, "key", path)?, path)?;
    let (kind_name, kind) = parse_pending_kind(take_required(&mut fields, "kind", path)?)?;
    let revision = expect_uint(&take_required(&mut fields, "origin_revision", path)?, path)?;
    let origin = expect_uint(&take_required(&mut fields, "origin_time", path)?, path)?;
    let owner_value = take_required(&mut fields, "owner", path)?;
    let owner_bytes = cbor_decode::encode(&owner_value);
    let owner = parse_stable_name(owner_value, counters, path)?;
    let stimulus = (
        expect_uint(&take_required(&mut fields, "stimulus_time", path)?, path)?,
        expect_uint(&take_required(&mut fields, "stimulus_order", path)?, path)?,
    );
    Ok(PendingEntry {
        stimulus,
        key,
        owner_bytes,
        owner,
        kind_name,
        kind,
        origin,
        deadline,
        revision,
        cause,
    })
}

fn parse_pending_kind<D>(value: Value) -> Result<(&'static str, PendingKind), DecodeFailure<D>> {
    let path = "payload.lifecycle.pending_events.kind";
    let (name, body) = expect_variant(value, path)?;
    let mut fields = into_fields(body, path)?;
    match name.as_str() {
        "pulse_delay_group" => {
            reject_unknown_fields(&fields, &["count"], path)?;
            let count = expect_uint(&take_required(&mut fields, "count", path)?, path)?;
            Ok(("pulse_delay_group", PendingKind::Pulse { count }))
        }
        "transport_transition" => {
            reject_unknown_fields(&fields, &["origin_time", "target"], path)?;
            let origin = expect_uint(&take_required(&mut fields, "origin_time", path)?, path)?;
            let target = expect_level(take_required(&mut fields, "target", path)?, path)?;
            Ok((
                "transport_transition",
                PendingKind::Transport { origin, target },
            ))
        }
        "inertial_maturation" => {
            reject_unknown_fields(&fields, &["qualification_origin", "target"], path)?;
            let origin = expect_uint(
                &take_required(&mut fields, "qualification_origin", path)?,
                path,
            )?;
            let target = expect_level(take_required(&mut fields, "target", path)?, path)?;
            Ok((
                "inertial_maturation",
                PendingKind::Inertial { origin, target },
            ))
        }
        "periodic_boundary" => {
            reject_unknown_fields(&fields, &["anchor", "ordinal"], path)?;
            let anchor = expect_uint(&take_required(&mut fields, "anchor", path)?, path)?;
            let ordinal = expect_uint(&take_required(&mut fields, "ordinal", path)?, path)?;
            Ok((
                "periodic_boundary",
                PendingKind::Periodic { anchor, ordinal },
            ))
        }
        _ => Err(unknown_variant(path, &name)),
    }
}

fn parse_provenance<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<ProvenanceSection, DecodeFailure<D>> {
    let path = "payload.provenance";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &[
            "episodes",
            "external_inputs",
            "output_baselines",
            "pending_events",
            "records",
            "state",
        ],
        path,
    )?;
    let episodes = parse_digest_list(
        take_required(&mut fields, "episodes", path)?,
        "payload.provenance.episodes",
    )?;
    let external_inputs = parse_digest_list(
        take_required(&mut fields, "external_inputs", path)?,
        "payload.provenance.external_inputs",
    )?;
    let output_baselines = parse_digest_list(
        take_required(&mut fields, "output_baselines", path)?,
        "payload.provenance.output_baselines",
    )?;
    let pending_events = parse_digest_list(
        take_required(&mut fields, "pending_events", path)?,
        "payload.provenance.pending_events",
    )?;
    let records = parse_records(take_required(&mut fields, "records", path)?, counters)?;
    let state = parse_digest_list(
        take_required(&mut fields, "state", path)?,
        "payload.provenance.state",
    )?;
    Ok(ProvenanceSection {
        episodes,
        external_inputs,
        output_baselines,
        pending_events,
        state,
        records,
    })
}

fn parse_digest_list<D>(value: Value, path: &str) -> Result<Vec<[u8; 32]>, DecodeFailure<D>> {
    let items = expect_array(value, path)?;
    let mut digests = Vec::with_capacity(items.len());
    let mut previous = None;
    for item in items {
        let digest = expect_digest(item, path)?;
        strict_increase(&mut previous, &digest, path)?;
        digests.push(digest);
    }
    Ok(digests)
}

fn parse_records<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<Vec<ParsedRecord>, DecodeFailure<D>> {
    let path = "payload.provenance.records";
    let items = expect_array(value, path)?;
    let mut records = Vec::with_capacity(items.len());
    let mut previous = None;
    let mut consumed = 0;
    for item in items {
        charge(
            &mut consumed,
            counters.policy.provenance_records(),
            "provenance_records",
        )?;
        let record = parse_record(item, counters)?;
        nondecreasing(&mut previous, &record.digest, path)?;
        records.push(record);
    }
    Ok(records)
}

fn parse_record<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<ParsedRecord, DecodeFailure<D>> {
    let path = "payload.provenance.records";
    let bytes = cbor_decode::encode(&value);
    let digest = *blake3::hash(&domain_separated(PROVENANCE_RECORD_DOMAIN, 2, &bytes)).as_bytes();
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &[
            "kind",
            "predecessors",
            "provenance_semantics_version",
            "revision",
            "subject",
            "time",
            "reaction_order",
            "stimulus_time",
        ],
        path,
    )?;
    let version = expect_uint(
        &take_required(&mut fields, "provenance_semantics_version", path)?,
        path,
    )?;
    if version != SUPPORTED_VERSION {
        return Err(unsupported_version(
            "provenance_record",
            "provenance_semantics_version",
            version,
        ));
    }
    let kind = parse_record_kind(take_required(&mut fields, "kind", path)?)?;
    let mut inapplicable = false;
    let predecessors = match take_optional(&mut fields, "predecessors") {
        Some(value) => {
            let parsed = parse_predecessors(value, counters)?;
            if parsed.is_empty() || predecessors_inapplicable(&parsed) {
                inapplicable = true;
            }
            parsed
        }
        None => Vec::new(),
    };
    let revision = match take_optional(&mut fields, "revision") {
        Some(value) => Some(expect_uint(&value, path)?),
        None => None,
    };
    let subject = match take_optional(&mut fields, "subject") {
        Some(value) => Some(parse_subject(value, counters, path)?),
        None => None,
    };
    let time = match take_optional(&mut fields, "time") {
        Some(value) => Some(expect_uint(&value, path)?),
        None => None,
    };
    let order = take_optional(&mut fields, "reaction_order")
        .map(|v| expect_uint(&v, path))
        .transpose()?;
    let stimulus_time = take_optional(&mut fields, "stimulus_time")
        .map(|v| expect_uint(&v, path))
        .transpose()?;
    let stamped = matches!(
        kind,
        RecordKind::Initialization
            | RecordKind::Ready
            | RecordKind::TopologyChange { .. }
            | RecordKind::ExternalObservation { .. }
            | RecordKind::ExternalPulse { .. }
            | RecordKind::PendingPulse { .. }
            | RecordKind::PendingTransport { .. }
            | RecordKind::PendingInertial { .. }
            | RecordKind::PendingPeriodic { .. }
    );
    if stamped != order.is_some()
        || stamped != stimulus_time.is_some()
        || (stamped && time.is_none())
    {
        return Err(malformed(path, "reaction stamp"));
    }
    inapplicable |= record_fields_inapplicable(
        &kind,
        revision.is_some(),
        subject.is_some(),
        time.is_some(),
        !predecessors.is_empty(),
    );
    if record_fields_missing(&kind, revision.is_some(), subject.is_some(), time.is_some()) {
        return Err(malformed(path, "record fields"));
    }
    Ok(ParsedRecord {
        bytes,
        digest,
        kind,
        predecessors,
        revision,
        subject,
        time,
        order,
        stimulus_time,
        inapplicable,
    })
}

fn record_fields_missing(kind: &RecordKind, revision: bool, subject: bool, time: bool) -> bool {
    match kind {
        RecordKind::Initialization | RecordKind::Ready => !revision || !time,
        RecordKind::TopologyChange { .. } => !revision || !time,
        RecordKind::Migration { .. } => !subject,
        RecordKind::Checkpoint { .. } => false,
        RecordKind::ExternalObservation { .. }
        | RecordKind::ExternalPulse { .. }
        | RecordKind::Derived
        | RecordKind::PulseDerived { .. }
        | RecordKind::PulseControlled { .. } => !subject,
        RecordKind::PendingPulse { .. }
        | RecordKind::PendingTransport { .. }
        | RecordKind::PendingInertial { .. }
        | RecordKind::PendingPeriodic { .. } => !revision || !subject || !time,
    }
}

fn record_fields_inapplicable(
    kind: &RecordKind,
    revision: bool,
    subject: bool,
    time: bool,
    predecessors: bool,
) -> bool {
    match kind {
        RecordKind::Initialization | RecordKind::Ready => subject || predecessors,
        RecordKind::TopologyChange { .. } => subject,
        RecordKind::Migration { .. } => revision || time,
        RecordKind::Checkpoint { .. } => revision || subject || time,
        RecordKind::ExternalObservation { .. } | RecordKind::ExternalPulse { .. } => {
            revision || predecessors
        }
        RecordKind::Derived
        | RecordKind::PulseDerived { .. }
        | RecordKind::PulseControlled { .. } => revision || time,
        RecordKind::PendingPulse { .. }
        | RecordKind::PendingTransport { .. }
        | RecordKind::PendingInertial { .. }
        | RecordKind::PendingPeriodic { .. } => false,
    }
}

fn predecessors_inapplicable(predecessors: &[Predecessor]) -> bool {
    predecessors
        .iter()
        .any(|predecessor| match &predecessor.role {
            PredecessorRole::Supporter => predecessor.contribution.is_some(),
            PredecessorRole::Contribution => predecessor.contribution.is_none(),
            PredecessorRole::Other(_) => false,
        })
}

fn parse_predecessors<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<Vec<Predecessor>, DecodeFailure<D>> {
    let path = "payload.provenance.records.predecessors";
    let items = expect_array(value, path)?;
    let mut predecessors = Vec::with_capacity(items.len());
    for item in items {
        charge(
            &mut counters.edges,
            counters.policy.provenance_edges(),
            "provenance_edges",
        )?;
        predecessors.push(parse_predecessor(item, counters)?);
    }
    Ok(predecessors)
}

fn parse_predecessor<D>(
    value: Value,
    counters: &mut Counters,
) -> Result<Predecessor, DecodeFailure<D>> {
    let path = "payload.provenance.records.predecessors";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(&fields, &["payload", "predecessor", "role"], path)?;
    let payload = take_required(&mut fields, "payload", path)?;
    let digest = expect_digest(take_required(&mut fields, "predecessor", path)?, path)?;
    let role_name = expect_text(take_required(&mut fields, "role", path)?, path)?;
    let (role, contribution) = match role_name.as_str() {
        "supporter" => (
            PredecessorRole::Supporter,
            supporter_payload(payload, path)?,
        ),
        "contribution" => (
            PredecessorRole::Contribution,
            contribution_payload(payload, counters, path)?,
        ),
        _ => (PredecessorRole::Other(role_name), other_payload(payload)),
    };
    Ok(Predecessor {
        role,
        digest,
        contribution,
    })
}

fn supporter_payload<D>(
    value: Value,
    path: &str,
) -> Result<Option<Contribution>, DecodeFailure<D>> {
    // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "events-episodes-and-provenance"
    // A supporter predecessor carries no contribution payload.
    if matches!(value, Value::Null) {
        Ok(None)
    } else {
        Err(malformed(path, "supporter payload"))
    }
}

fn other_payload(value: Value) -> Option<Contribution> {
    let _ = value;
    None
}

fn contribution_payload<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<Option<Contribution>, DecodeFailure<D>> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    Ok(Some(parse_contribution(value, counters, path)?))
}

fn parse_contribution<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<Contribution, DecodeFailure<D>> {
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(&fields, &["count", "port"], path)?;
    let count = expect_uint(&take_required(&mut fields, "count", path)?, path)?;
    let port = parse_port(take_required(&mut fields, "port", path)?, counters, path)?;
    Ok(Contribution { count, port })
}

fn parse_port<D>(
    value: Value,
    counters: &mut Counters,
    path: &str,
) -> Result<PortName, DecodeFailure<D>> {
    let (name, body) = expect_variant(value, path)?;
    match name.as_str() {
        "in_port" => Ok(PortName::Top(expect_key(body, path)?)),
        "module_in_port" => {
            let mut fields = into_fields(body, path)?;
            reject_unknown_fields(&fields, &["instances", "port"], path)?;
            let instances = parse_instance_path(
                take_required(&mut fields, "instances", path)?,
                counters,
                path,
            )?;
            if instances.is_empty() {
                return Err(malformed(path, "empty instance path"));
            }
            let port = expect_key(take_required(&mut fields, "port", path)?, path)?;
            Ok(PortName::Module { instances, port })
        }
        _ => Err(unknown_variant(path, &name)),
    }
}

fn parse_record_kind<D>(value: Value) -> Result<RecordKind, DecodeFailure<D>> {
    let path = "payload.provenance.records.kind";
    let (name, body) = expect_variant(value, path)?;
    match name.as_str() {
        "topology_change" => {
            let mut fields = into_fields(body, path)?;
            reject_unknown_fields(&fields, &["base", "target"], path)?;
            Ok(RecordKind::TopologyChange {
                base: expect_digest(take_required(&mut fields, "base", path)?, path)?,
                target: expect_digest(take_required(&mut fields, "target", path)?, path)?,
            })
        }
        "migration" => Ok(RecordKind::Migration {
            rule: expect_text(body, path)?,
        }),
        "checkpoint" => match body {
            Value::Bytes(fact) => Ok(RecordKind::Checkpoint { fact }),
            _ => Err(malformed(path, "checkpoint bytes")),
        },
        "initialization_transaction" => {
            require_null(body, path)?;
            Ok(RecordKind::Initialization)
        }
        "ready_transaction" => {
            require_null(body, path)?;
            Ok(RecordKind::Ready)
        }
        "external_observation" => Ok(RecordKind::ExternalObservation {
            value: expect_level(body, path)?,
        }),
        "external_pulse_observation" => Ok(RecordKind::ExternalPulse {
            count: expect_uint(&body, path)?,
        }),
        "pending_pulse_delay" => Ok(RecordKind::PendingPulse {
            count: record_count(body, path)?,
        }),
        "pending_transport_delay" => Ok(RecordKind::PendingTransport {
            target: record_target(body, path)?,
        }),
        "pending_inertial_delay" => Ok(RecordKind::PendingInertial {
            target: record_target(body, path)?,
        }),
        "pending_periodic_boundary" => parse_periodic_record(body),
        "derived" => {
            require_null(body, path)?;
            Ok(RecordKind::Derived)
        }
        "pulse_derived" => Ok(RecordKind::PulseDerived {
            result: expect_uint(&body, path)?,
        }),
        "pulse_controlled_level" => Ok(RecordKind::PulseControlled {
            result: expect_level(body, path)?,
        }),
        _ => Err(unknown_variant(path, &name)),
    }
}

fn record_count<D>(value: Value, path: &str) -> Result<u64, DecodeFailure<D>> {
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(&fields, &["count"], path)?;
    expect_uint(&take_required(&mut fields, "count", path)?, path)
}

fn record_target<D>(value: Value, path: &str) -> Result<LogicLevel, DecodeFailure<D>> {
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(&fields, &["target"], path)?;
    expect_level(take_required(&mut fields, "target", path)?, path)
}

fn parse_periodic_record<D>(value: Value) -> Result<RecordKind, DecodeFailure<D>> {
    let path = "payload.provenance.records.kind";
    let mut fields = into_fields(value, path)?;
    reject_unknown_fields(
        &fields,
        &["anchor", "first_emission", "ordinal", "reenable_phase"],
        path,
    )?;
    let anchor = expect_uint(&take_required(&mut fields, "anchor", path)?, path)?;
    let first_emission = parse_first_emission(take_required(&mut fields, "first_emission", path)?)?;
    let ordinal = expect_uint(&take_required(&mut fields, "ordinal", path)?, path)?;
    let reenable_phase = parse_reenable_phase(take_required(&mut fields, "reenable_phase", path)?)?;
    Ok(RecordKind::PendingPeriodic {
        anchor,
        ordinal,
        first_emission,
        reenable_phase,
    })
}

fn parse_first_emission<D>(value: Value) -> Result<FirstEmissionPolicy, DecodeFailure<D>> {
    let path = "payload.provenance.records.kind.first_emission";
    let (name, body) = named_variant(value, &["after_first_period", "immediate"], path)?;
    require_null(body, path)?;
    Ok(match name.as_str() {
        "immediate" => FirstEmissionPolicy::Immediate,
        _ => FirstEmissionPolicy::AfterFirstPeriod,
    })
}

fn parse_reenable_phase<D>(value: Value) -> Result<ReenablePhasePolicy, DecodeFailure<D>> {
    let path = "payload.provenance.records.kind.reenable_phase";
    let (name, body) = named_variant(value, &["preserve_phase", "restart_phase"], path)?;
    require_null(body, path)?;
    Ok(match name.as_str() {
        "restart_phase" => ReenablePhasePolicy::RestartPhase,
        _ => ReenablePhasePolicy::PreservePhase,
    })
}

struct Installed {
    edges: Vec<EdgeObservation>,
    stored: Vec<LogicLevel>,
    anchors: BTreeMap<NodeKey, InstalledPhase>,
    levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    baselines: BTreeMap<ExternalOutputKey<Level>, (LogicLevel, [u8; 32])>,
    pending: Vec<ResolvedPending>,
    singular: Vec<SingularRef>,
}

struct InstalledPhase {
    anchor: u64,
    origin: (u64, u64),
    settled: Option<u64>,
}

impl InstalledPhase {
    fn runtime<D>(&self) -> crate::machine::PeriodicPhase<D> {
        crate::machine::PeriodicPhase {
            anchor: Time::from_ticks(self.anchor),
            origin: crate::ReactionStamp::from_parts(
                Time::from_ticks(self.origin.0),
                self.origin.1,
            ),
            settled: self.settled.map(Time::from_ticks),
        }
    }
}

#[derive(Clone, Copy)]
struct ResolvedPending {
    stimulus: (u64, u64),
    key: u64,
    node: NodeKey,
    kind: PendingKind,
    node_kind: &'static str,
    origin: u64,
    deadline: u64,
    revision: u64,
    cause: [u8; 32],
}

#[derive(Clone, Copy)]
struct SingularRef {
    node: NodeKey,
    kind: &'static str,
    event: Option<u64>,
}

struct CheckedEpisode<D> {
    began_order: u64,
    changed_order: u64,
    identity: DiagnosticEpisodeId,
    condition: DiagnosticConditionKey,
    problem: Problem<D>,
    began: u64,
    changed: u64,
    cause: [u8; 32],
}

struct RestoredProvenance<D> {
    view: Option<ProvenanceView<D>>,
    episodes: BTreeMap<DiagnosticConditionKey, ActiveDiagnosticEpisode<D>>,
    inputs: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    outputs: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    edges: BTreeMap<NodeKey, CauseRef>,
    toggles: BTreeMap<NodeKey, CauseRef>,
    establishments: BTreeMap<NodeKey, CauseRef>,
    transports: BTreeMap<NodeKey, CauseRef>,
    inertial_cancels: BTreeMap<NodeKey, CauseRef>,
    anchors: BTreeMap<NodeKey, CauseRef>,
    periodic_cancels: BTreeMap<NodeKey, CauseRef>,
    pending: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    operation: Option<CauseRef>,
}

#[derive(Clone, Copy)]
enum ExpectedSlot {
    Edge(usize),
    Stored(usize),
    Remembered { remembered: usize, output: usize },
    Periodic(usize),
    Pulse,
    Transport,
    Inertial,
    PeriodicEvent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CauseFamily {
    Edge,
    Toggle,
    Establishment,
    Transport,
    Inertial,
    Periodic,
    Pulse,
}

fn fail_state<D>(owner: &str, expected: &str, encountered: &str) -> RestoreFailure<D> {
    RestoreFailure::StateSchemaMismatch(persistence_problem(
        ProblemEvidence::PersistenceStateSchemaMismatch {
            evidence: StateSchemaEvidence {
                owner: owner.to_owned(),
                expected: expected.to_owned(),
                encountered: encountered.to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn fail_subject<D>(subject: &str) -> RestoreFailure<D> {
    RestoreFailure::UnknownSubject(persistence_problem(
        ProblemEvidence::PersistenceUnknownSubject {
            evidence: MissingSubjectEvidence {
                subject: subject.to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn fail_lifecycle<D>() -> RestoreFailure<D> {
    RestoreFailure::LifecycleShapeInvalid(persistence_problem(
        ProblemEvidence::PersistenceLifecycleShapeInvalid {
            evidence: ParameterEvidence {
                owner: None,
                parameter: "lifecycle",
                expected_domain: "snapshot lifecycle",
                encountered: None,
                operands: Vec::new(),
            },
            marker: PhantomData,
        },
    ))
}

fn fail_revision<D>(expected: u64, actual: u64) -> RestoreFailure<D> {
    restore_identity(
        RestoreFailure::TopologyRevisionMismatch,
        "topology_revision",
        &expected.to_string(),
        &actual.to_string(),
    )
}

fn fail_pending<D>(event: Option<u64>, owner: &str, detail: &str) -> RestoreFailure<D> {
    RestoreFailure::PendingEventInvalid(persistence_problem(
        ProblemEvidence::PersistencePendingEventInvalid {
            evidence: PendingEventEvidence {
                event,
                owner: owner.to_owned(),
                origin: None,
                deadline: None,
                detail: detail.to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn fail_event_link<D>(event: Option<u64>, owner: &str) -> RestoreFailure<D> {
    RestoreFailure::EventIdentityStateInvalid(persistence_problem(
        ProblemEvidence::PersistenceEventIdentityStateInvalid {
            evidence: PendingEventEvidence {
                event,
                owner: owner.to_owned(),
                origin: None,
                deadline: None,
                detail: "singular temporal reference".to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn fail_episode<D>(episode: &EpisodeEntry) -> RestoreFailure<D> {
    RestoreFailure::DiagnosticEpisodeInvalid(persistence_problem(
        ProblemEvidence::PersistenceDiagnosticEpisodeInvalid {
            evidence: episode_evidence_text(episode),
            marker: PhantomData,
        },
    ))
}

fn fail_episode_schema<D>(episode: &EpisodeEntry) -> RestoreFailure<D> {
    RestoreFailure::DiagnosticSchemaInvalid(persistence_problem(
        ProblemEvidence::PersistenceDiagnosticSchemaInvalid {
            evidence: episode_evidence_text(episode),
            marker: PhantomData,
        },
    ))
}

fn episode_evidence_text(episode: &EpisodeEntry) -> DiagnosticEpisodeEvidence {
    DiagnosticEpisodeEvidence {
        identity: hex(&episode.identity),
        code: episode.code.clone(),
        owner: hex(&episode.owner.node.to_be_bytes()),
        discriminator: Some(episode.discriminator),
        began_at: Some(episode.began_at),
        last_material_change: Some(episode.last_material_change),
    }
}

fn fail_settled<D>(detail: &str) -> RestoreFailure<D> {
    RestoreFailure::SettledStateInconsistent(persistence_problem(
        ProblemEvidence::PersistenceSettledStateInconsistent {
            evidence: SettledStateEvidence {
                detail: detail.to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn digest_evidence(kind: &'static str, expected: &[u8], actual: &[u8]) -> DigestMismatchEvidence {
    DigestMismatchEvidence {
        kind,
        expected: hex(expected),
        actual: hex(actual),
        context: "restore".to_owned(),
    }
}

fn fail_execution_digest<D>(expected: &[u8], actual: &[u8]) -> RestoreFailure<D> {
    RestoreFailure::ExecutionDigestMismatch(persistence_problem(
        ProblemEvidence::PersistenceExecutionDigestMismatch {
            evidence: digest_evidence("execution", expected, actual),
            marker: PhantomData,
        },
    ))
}

fn fail_observable_digest<D>(expected: &[u8], actual: &[u8]) -> RestoreFailure<D> {
    RestoreFailure::ObservableDigestMismatch(persistence_problem(
        ProblemEvidence::PersistenceObservableDigestMismatch {
            evidence: digest_evidence("observable", expected, actual),
            marker: PhantomData,
        },
    ))
}

fn fail_snapshot_digest<D>(expected: &[u8], actual: &[u8]) -> RestoreFailure<D> {
    RestoreFailure::SnapshotDigestMismatch(persistence_problem(
        ProblemEvidence::PersistenceSnapshotDigestMismatch {
            evidence: digest_evidence("snapshot", expected, actual),
            marker: PhantomData,
        },
    ))
}

fn provenance_evidence(
    fault: GraphFault,
    digest: &str,
    kind: &str,
    detail: &str,
) -> PersistenceProvenanceEvidence {
    let mut evidence = PersistenceProvenanceEvidence {
        digest: digest.to_owned(),
        record_kind: kind.to_owned(),
        subject: String::new(),
        predecessor: String::new(),
        role: String::new(),
        root: String::new(),
        checkpoint: String::new(),
        conflict: String::new(),
    };
    let slot = match fault {
        GraphFault::Missing => &mut evidence.predecessor,
        GraphFault::Role => &mut evidence.role,
        GraphFault::Closure => &mut evidence.root,
        GraphFault::Checkpoint => &mut evidence.checkpoint,
        GraphFault::Conflict => &mut evidence.conflict,
        GraphFault::Digest | GraphFault::Cycle | GraphFault::Subject => &mut evidence.subject,
    };
    *slot = detail.to_owned();
    evidence
}

#[derive(Clone, Copy)]
enum GraphFault {
    Missing,
    Digest,
    Cycle,
    Subject,
    Role,
    Closure,
    Conflict,
    Checkpoint,
}

fn fail_graph<D>(fault: GraphFault, digest: &str, kind: &str, detail: &str) -> RestoreFailure<D> {
    let evidence = provenance_evidence(fault, digest, kind, detail);
    let problem = persistence_problem(match fault {
        GraphFault::Missing => ProblemEvidence::PersistenceProvenanceMissingPredecessor {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Digest => ProblemEvidence::PersistenceProvenanceDigestMismatch {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Cycle => ProblemEvidence::PersistenceProvenanceCycle {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Subject => ProblemEvidence::PersistenceProvenanceInvalidSubject {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Role => ProblemEvidence::PersistenceProvenanceInvalidRole {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Closure => ProblemEvidence::PersistenceProvenanceIncompleteRootClosure {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Conflict => ProblemEvidence::PersistenceProvenanceConflictingRecord {
            evidence,
            marker: PhantomData,
        },
        GraphFault::Checkpoint => ProblemEvidence::PersistenceProvenanceFalseCheckpoint {
            evidence,
            marker: PhantomData,
        },
    });
    match fault {
        GraphFault::Missing => RestoreFailure::ProvenanceMissingPredecessor(problem),
        GraphFault::Digest => RestoreFailure::ProvenanceDigestMismatch(problem),
        GraphFault::Cycle => RestoreFailure::ProvenanceCycle(problem),
        GraphFault::Subject => RestoreFailure::ProvenanceInvalidSubject(problem),
        GraphFault::Role => RestoreFailure::ProvenanceInvalidRole(problem),
        GraphFault::Closure => RestoreFailure::ProvenanceIncompleteRootClosure(problem),
        GraphFault::Conflict => RestoreFailure::ProvenanceConflictingRecord(problem),
        GraphFault::Checkpoint => RestoreFailure::ProvenanceFalseCheckpoint(problem),
    }
}

fn fail_collision<D>(digest: &[u8], first: &[u8], second: &[u8]) -> RestoreFailure<D> {
    RestoreFailure::DigestCollision(persistence_problem(
        ProblemEvidence::PersistenceDigestCollision {
            evidence: DigestCollisionEvidence {
                kind: "provenance",
                domain: PROVENANCE_RECORD_DOMAIN,
                digest: hex(digest),
                first: hex(first),
                second: hex(second),
                context: "provenance record".to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn check_standard_expansion<D>(compiled: &CompiledNetwork<D>) -> Result<(), RestoreFailure<D>> {
    for (_, module) in compiled.standard_modules() {
        if let Err(error) = crate::standard::retained_expansion_agrees(module) {
            return Err(standard_failure(error));
        }
    }
    Ok(())
}

fn standard_failure<D>(error: RetainedExpansionError) -> RestoreFailure<D> {
    match error {
        RetainedExpansionError::Unsupported(module_ref) => {
            RestoreFailure::StandardUnsupportedVersion(standard_problem(
                ProblemEvidence::standard_module_unsupported_version(module_ref),
            ))
        }
        RetainedExpansionError::Interface(module_ref) => {
            RestoreFailure::StandardInterfaceMismatch(standard_problem(
                ProblemEvidence::standard_module_interface_mismatch(module_ref, Vec::new()),
            ))
        }
        RetainedExpansionError::Mismatch(module_ref, detail) => {
            RestoreFailure::StandardExpansionMismatch(standard_problem(
                ProblemEvidence::StandardModuleExpansionMismatch {
                    module_ref,
                    detail,
                    marker: PhantomData,
                },
            ))
        }
    }
}

struct Planned {
    slot: ExpectedSlot,
    kind: &'static str,
}

fn install_state<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
) -> Result<Installed, RestoreFailure<D>> {
    if matches!(artifact.lifecycle, Lifecycle::Awaiting)
        && (!artifact.episodes.is_empty() || !artifact.provenance.records.is_empty())
    {
        return Err(fail_lifecycle());
    }
    let plan = state_plan(compiled);
    let mut placed = BTreeMap::new();
    place_table(compiled, artifact, &artifact.node_state, &plan, &mut placed)?;
    place_table(
        compiled,
        artifact,
        &artifact.temporal_state,
        &plan,
        &mut placed,
    )?;
    if placed.len() != plan.len() {
        return Err(fail_state("state", "complete table", "missing entry"));
    }
    materialize(compiled, artifact, &plan, &placed)
}

fn state_plan<D>(compiled: &CompiledNetwork<D>) -> BTreeMap<(NodeKey, &'static str), Planned> {
    let mut plan = BTreeMap::new();
    for node in compiled.snapshot_nodes() {
        let mut insert = |schema: &'static str, slot: ExpectedSlot| {
            plan.insert(
                (node.flat, schema),
                Planned {
                    slot,
                    kind: node.kind,
                },
            );
        };
        match node.family {
            SnapshotNodeFamily::Edge { index } => {
                insert("edge_observation", ExpectedSlot::Edge(index))
            }
            SnapshotNodeFamily::StoredLevel { index } => {
                insert("stored_level", ExpectedSlot::Stored(index));
            }
            SnapshotNodeFamily::PulseDelay => insert("pending_pulse_group", ExpectedSlot::Pulse),
            SnapshotNodeFamily::Transport { remembered, output } => {
                insert(
                    "remembered_input_output",
                    ExpectedSlot::Remembered { remembered, output },
                );
                insert("pending_transport_transition", ExpectedSlot::Transport);
            }
            SnapshotNodeFamily::Inertial { remembered, output } => {
                insert(
                    "remembered_input_output",
                    ExpectedSlot::Remembered { remembered, output },
                );
                insert("pending_inertial_candidate", ExpectedSlot::Inertial);
            }
            SnapshotNodeFamily::Periodic { previous_enable } => {
                insert(
                    "periodic_anchor_previous_enable",
                    ExpectedSlot::Periodic(previous_enable),
                );
                insert("pending_periodic_boundary", ExpectedSlot::PeriodicEvent);
            }
        }
    }
    plan
}

fn place_table<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    entries: &[StateEntry],
    plan: &BTreeMap<(NodeKey, &'static str), Planned>,
    placed: &mut BTreeMap<(NodeKey, &'static str), ParsedState>,
) -> Result<(), RestoreFailure<D>> {
    let awaiting = matches!(artifact.lifecycle, Lifecycle::Awaiting);
    let initial_edges = compiled.initial_edge_observations();
    let initial_stored = compiled.initial_stored_levels();
    for entry in entries {
        let node = resolve_name(compiled, &entry.owner)?;
        let Some(schema) = static_name(STATE_SCHEMAS, &entry.schema) else {
            return Err(fail_state(
                &owner_label(&entry.owner),
                "known schema",
                &entry.schema,
            ));
        };
        let Some(planned) = plan.get(&(node, schema)) else {
            return Err(fail_state(
                &owner_label(&entry.owner),
                "declared schema",
                schema,
            ));
        };
        if entry.kind != planned.kind {
            return Err(fail_state(
                &owner_label(&entry.owner),
                planned.kind,
                &entry.kind,
            ));
        }
        if placed.insert((node, schema), entry.value).is_some() {
            return Err(fail_state(&owner_label(&entry.owner), schema, "duplicate"));
        }
        if !shape_matches(planned.slot, entry.value) {
            return Err(fail_state(&owner_label(&entry.owner), schema, "value"));
        }
        if awaiting && !initial_matches(planned.slot, entry.value, &initial_edges, &initial_stored)
        {
            return Err(fail_state(
                &owner_label(&entry.owner),
                "declared initial",
                schema,
            ));
        }
    }
    Ok(())
}

fn static_name(names: &[&'static str], name: &str) -> Option<&'static str> {
    names.iter().copied().find(|candidate| *candidate == name)
}

fn shape_matches(slot: ExpectedSlot, value: ParsedState) -> bool {
    matches!(
        (slot, value),
        (ExpectedSlot::Edge(_), ParsedState::Edge(_))
            | (ExpectedSlot::Stored(_), ParsedState::Level(_))
            | (
                ExpectedSlot::Remembered { .. },
                ParsedState::Remembered { .. }
            )
            | (ExpectedSlot::Periodic(_), ParsedState::Periodic { .. })
            | (
                ExpectedSlot::Pulse | ExpectedSlot::Transport,
                ParsedState::Temporal(None)
            )
            | (
                ExpectedSlot::Inertial | ExpectedSlot::PeriodicEvent,
                ParsedState::Temporal(_)
            )
    )
}

fn initial_matches(
    slot: ExpectedSlot,
    value: ParsedState,
    edges: &[EdgeObservation],
    stored: &[LogicLevel],
) -> bool {
    match (slot, value) {
        (ExpectedSlot::Edge(index), ParsedState::Edge(observation)) => {
            edges.get(index) == Some(&observation)
        }
        (ExpectedSlot::Stored(index), ParsedState::Level(level)) => {
            stored.get(index) == Some(&level)
        }
        (
            ExpectedSlot::Remembered { remembered, output },
            ParsedState::Remembered {
                remembered: remembered_level,
                output: output_level,
            },
        ) => {
            stored.get(remembered) == Some(&remembered_level)
                && stored.get(output) == Some(&output_level)
        }
        (
            ExpectedSlot::Periodic(index),
            ParsedState::Periodic {
                anchor: None,
                previous,
                ..
            },
        ) => stored.get(index) == Some(&previous),
        (ExpectedSlot::Pulse | ExpectedSlot::Transport, ParsedState::Temporal(None))
        | (ExpectedSlot::Inertial | ExpectedSlot::PeriodicEvent, ParsedState::Temporal(None)) => {
            true
        }
        _ => false,
    }
}

fn materialize<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    plan: &BTreeMap<(NodeKey, &'static str), Planned>,
    placed: &BTreeMap<(NodeKey, &'static str), ParsedState>,
) -> Result<Installed, RestoreFailure<D>> {
    let mut installed = Installed {
        edges: compiled.initial_edge_observations(),
        stored: compiled.initial_stored_levels(),
        anchors: BTreeMap::new(),
        levels: BTreeMap::new(),
        baselines: BTreeMap::new(),
        pending: Vec::new(),
        singular: Vec::new(),
    };
    for ((node, schema), value) in placed {
        let Some(planned) = plan.get(&(*node, *schema)) else {
            return Err(fail_state(&node_label(*node), "declared schema", schema));
        };
        if !apply_slot(&mut installed, *node, planned.slot, *value) {
            return Err(fail_state(&node_label(*node), schema, "slot"));
        }
    }
    if let Lifecycle::Ready(ready) = &artifact.lifecycle {
        install_ready_facts(compiled, ready, &mut installed)?;
    }
    Ok(installed)
}

fn apply_slot(
    installed: &mut Installed,
    node: NodeKey,
    slot: ExpectedSlot,
    value: ParsedState,
) -> bool {
    match (slot, value) {
        (ExpectedSlot::Edge(index), ParsedState::Edge(observation)) => {
            write_edge(&mut installed.edges, index, observation)
        }
        (ExpectedSlot::Stored(index), ParsedState::Level(level)) => {
            write_stored(&mut installed.stored, index, level)
        }
        (
            ExpectedSlot::Remembered { remembered, output },
            ParsedState::Remembered {
                remembered: remembered_level,
                output: output_level,
            },
        ) => {
            write_stored(&mut installed.stored, remembered, remembered_level)
                && write_stored(&mut installed.stored, output, output_level)
        }
        (
            ExpectedSlot::Periodic(index),
            ParsedState::Periodic {
                anchor,
                previous,
                phase,
                settled,
            },
        ) => {
            if !write_stored(&mut installed.stored, index, previous) {
                return false;
            }
            if let Some(anchor) = anchor {
                if let Some(phase) = phase {
                    installed.anchors.insert(
                        node,
                        InstalledPhase {
                            anchor,
                            origin: phase,
                            settled,
                        },
                    );
                } else {
                    return false;
                }
            }
            true
        }
        (ExpectedSlot::Inertial, ParsedState::Temporal(event)) => {
            installed.singular.push(SingularRef {
                node,
                kind: "inertial_delay",
                event,
            });
            true
        }
        (ExpectedSlot::PeriodicEvent, ParsedState::Temporal(event)) => {
            installed.singular.push(SingularRef {
                node,
                kind: "periodic",
                event,
            });
            true
        }
        (ExpectedSlot::Pulse | ExpectedSlot::Transport, ParsedState::Temporal(None)) => true,
        _ => false,
    }
}

fn write_edge(edges: &mut [EdgeObservation], index: usize, observation: EdgeObservation) -> bool {
    match edges.get_mut(index) {
        Some(slot) => {
            *slot = observation;
            true
        }
        None => false,
    }
}

fn write_stored(stored: &mut [LogicLevel], index: usize, level: LogicLevel) -> bool {
    match stored.get_mut(index) {
        Some(slot) => {
            *slot = level;
            true
        }
        None => false,
    }
}

fn install_ready_facts<D>(
    compiled: &CompiledNetwork<D>,
    ready: &LifecycleReady,
    installed: &mut Installed,
) -> Result<(), RestoreFailure<D>> {
    for (key, level) in &ready.levels {
        let input = ExternalInputKey::<Level>::from_u128(*key);
        if !compiled.contains_external_level_input(input) {
            return Err(fail_subject(&hex(&key.to_be_bytes())));
        }
        installed.levels.insert(input, *level);
    }
    for input in compiled.external_level_inputs() {
        if !installed.levels.contains_key(&input) {
            return Err(fail_lifecycle());
        }
    }
    for baseline in &ready.baselines {
        if !baseline.established {
            return Err(fail_lifecycle());
        }
        let output = ExternalOutputKey::<Level>::from_u128(baseline.key);
        if !compiled.contains_external_level_output(output) {
            return Err(fail_subject(&hex(&baseline.key.to_be_bytes())));
        }
        if installed
            .baselines
            .insert(output, (baseline.level, baseline.cause))
            .is_some()
        {
            return Err(fail_lifecycle());
        }
    }
    for output in compiled.external_level_outputs() {
        if !installed.baselines.contains_key(&output) {
            return Err(fail_lifecycle());
        }
    }
    Ok(())
}

fn check_pending<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    installed: &mut Installed,
) -> Result<(), RestoreFailure<D>> {
    let Lifecycle::Ready(ready) = &artifact.lifecycle else {
        return Ok(());
    };
    for (node, phase) in &installed.anchors {
        if phase.origin > (ready.time, ready.order)
            || phase
                .settled
                .is_some_and(|at| at > ready.time || at < phase.origin.0)
        {
            return Err(fail_lifecycle());
        }
        let Some((period, ..)) = compiled.periodic(*node) else {
            return Err(fail_lifecycle());
        };
        // SPEC: docs/specs/contracts/ordered-reactions.yaml "singular-inertial-and-once-only-phase"
        // A committed reaction at this cadence boundary has assessed it, even while disabled.
        if ready.time >= phase.anchor
            && (ready.time - phase.anchor) % period.ticks() == 0
            && phase.settled != Some(ready.time)
        {
            return Err(fail_lifecycle());
        }
    }
    let mut seen = BTreeSet::new();
    let mut transports = BTreeMap::new();
    for event in &ready.pending {
        if event.revision > artifact.revision.value() {
            return Err(fail_revision(artifact.revision.value(), event.revision));
        }
        let node = resolve_name(compiled, &event.owner)?;
        if let PendingKind::Transport { target, .. } = event.kind {
            if transports
                .insert((node, event.deadline, event.stimulus), target)
                .is_some_and(|previous| previous != target)
            {
                return Err(fail_pending(
                    Some(event.key),
                    &node_label(node),
                    "conflicting origin stamp",
                ));
            }
        }
        let node_kind = node_identity(compiled, node);
        let expected = pending_owner_kind(event.kind_name);
        if node_kind != Some(expected) {
            return Err(fail_pending(
                Some(event.key),
                &owner_label(&event.owner),
                "kind",
            ));
        }
        if event.stimulus > (ready.time, ready.order)
            || event.deadline <= ready.time
            || event.origin >= event.deadline
        {
            return Err(fail_pending(
                Some(event.key),
                &owner_label(&event.owner),
                "deadline",
            ));
        }
        if event.key >= artifact.next_serial || !seen.insert(event.key) {
            return Err(fail_pending(
                Some(event.key),
                &owner_label(&event.owner),
                "serial",
            ));
        }
        if !pending_origin_matches(event) {
            return Err(fail_pending(
                Some(event.key),
                &owner_label(&event.owner),
                "origin",
            ));
        }
        installed.pending.push(ResolvedPending {
            stimulus: event.stimulus,
            key: event.key,
            node,
            kind: event.kind,
            node_kind: expected,
            origin: event.origin,
            deadline: event.deadline,
            revision: event.revision,
            cause: event.cause,
        });
    }
    Ok(())
}

fn pending_owner_kind(kind: &str) -> &'static str {
    match kind {
        "pulse_delay_group" => "pulse_delay",
        "transport_transition" => "transport_delay",
        "inertial_maturation" => "inertial_delay",
        _ => "periodic",
    }
}

fn pending_origin_matches(event: &PendingEntry) -> bool {
    match event.kind {
        PendingKind::Transport { origin, .. } | PendingKind::Inertial { origin, .. } => {
            origin == event.origin
        }
        PendingKind::Pulse { .. } | PendingKind::Periodic { .. } => true,
    }
}

fn node_identity<D>(compiled: &CompiledNetwork<D>, node: NodeKey) -> Option<&'static str> {
    compiled
        .snapshot_nodes()
        .into_iter()
        .find(|snapshot| snapshot.flat == node)
        .map(|snapshot| snapshot.kind)
}

fn check_singular_references<D>(installed: &Installed) -> Result<(), RestoreFailure<D>> {
    for reference in &installed.singular {
        let serials: Vec<u64> = installed
            .pending
            .iter()
            .filter(|event| event.node == reference.node && event.node_kind == reference.kind)
            .map(|event| event.key)
            .collect();
        // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "events-episodes-and-provenance"
        // A singular candidate names exactly one pending event, or none.
        let valid = match reference.event {
            Some(serial) => serials.len() == 1 && serials.first() == Some(&serial),
            None => serials.is_empty(),
        };
        if !valid {
            return Err(fail_event_link(
                reference.event,
                &node_label(reference.node),
            ));
        }
    }
    Ok(())
}

fn check_episodes<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
) -> Result<Vec<CheckedEpisode<D>>, RestoreFailure<D>> {
    let mut episodes = Vec::new();
    let mut conditions = BTreeSet::new();
    for episode in &artifact.episodes {
        let owner = resolve_name(compiled, &episode.owner)?;
        if episode.revision > artifact.revision.value()
            || episode.evidence.revision > artifact.revision.value()
        {
            let actual = episode.revision.max(episode.evidence.revision);
            return Err(fail_revision(artifact.revision.value(), actual));
        }
        if DiagnosticCode::ALL
            .iter()
            .copied()
            .find(|code| code.as_str() == episode.code)
            != Some(DiagnosticCode::RuntimeLevelLatchConflictRetained)
        {
            return Err(fail_episode_schema(episode));
        }
        if let Lifecycle::Ready(ready) = &artifact.lifecycle {
            if (episode.last_material_change, episode.changed_order) > (ready.time, ready.order) {
                return Err(fail_episode(episode));
            }
        } else {
            return Err(fail_lifecycle());
        }
        let checked = checked_episode(compiled, episode, owner)?;
        if !conditions.insert(checked.condition.clone()) {
            return Err(fail_episode(episode));
        }
        episodes.push(checked);
    }
    Ok(episodes)
}

fn checked_episode<D>(
    compiled: &CompiledNetwork<D>,
    episode: &EpisodeEntry,
    _flat: NodeKey,
) -> Result<CheckedEpisode<D>, RestoreFailure<D>> {
    let Some(discriminator) = u8::try_from(episode.discriminator).ok() else {
        return Err(fail_episode(episode));
    };
    let (primary, evidence_node, condition_owner) = episode_owner(&episode.owner);
    let condition = DiagnosticConditionKey::restored(
        condition_owner,
        DiagnosticCode::RuntimeLevelLatchConflictRetained,
        discriminator,
    );
    let identity = DiagnosticEpisodeId::derive(
        compiled.network_key(),
        &condition,
        crate::ReactionStamp::<D>::from_parts(
            Time::from_ticks(episode.began_at),
            episode.began_order,
        ),
    );
    let controls_match = episode.evidence.controls
        == ConflictControls::Level {
            set: LogicLevel::High,
            reset: LogicLevel::High,
        };
    if discriminator != 0
        || identity.as_bytes() != episode.identity
        || episode.evidence.policy != ConflictPolicy::RetainAndDiagnose
        || !controls_match
        || episode.evidence.revision != episode.revision
        || (episode.last_material_change, episode.changed_order)
            < (episode.began_at, episode.began_order)
    {
        return Err(fail_episode(episode));
    }
    let problem = Problem::new(
        primary,
        Vec::new(),
        ProblemEvidence::RuntimeLevelLatchConflictRetained {
            evidence: ConflictEvidence {
                node: evidence_node,
                policy: episode.evidence.policy,
                previous: episode.evidence.previous,
                controls: episode.evidence.controls,
                reaction_order: episode.changed_order,
                at_ticks: episode.last_material_change,
                revision: NetworkRevision::from_value(episode.revision),
            },
            marker: PhantomData,
        },
    );
    Ok(CheckedEpisode {
        began_order: episode.began_order,
        changed_order: episode.changed_order,
        identity: DiagnosticEpisodeId::from_bytes(episode.identity),
        condition,
        problem,
        began: episode.began_at,
        changed: episode.last_material_change,
        cause: episode.cause,
    })
}

fn episode_owner(name: &StableName) -> (SubjectRef, NodeEvidence, NodeSubject) {
    if name.instances.is_empty() {
        let node = NodeKey::from_u128(name.node);
        (
            SubjectRef::Node(node),
            NodeEvidence::Node(node),
            NodeSubject::Node(node),
        )
    } else {
        let instances = instance_keys(&name.instances);
        let node = NodeKey::from_u128(name.node);
        let qualified = QualifiedNodeRef::new(instances.clone(), node);
        (
            SubjectRef::QualifiedNode(qualified.clone()),
            NodeEvidence::Qualified { instances, node },
            NodeSubject::Qualified(qualified),
        )
    }
}

fn resolve_name<D>(
    compiled: &CompiledNetwork<D>,
    name: &StableName,
) -> Result<NodeKey, RestoreFailure<D>> {
    match compiled.resolve_stable_node(
        &instance_keys(&name.instances),
        NodeKey::from_u128(name.node),
    ) {
        Some(node) => Ok(node),
        None => Err(fail_subject(&owner_label(name))),
    }
}

fn instance_keys(instances: &[u128]) -> Vec<ModuleInstanceKey> {
    instances
        .iter()
        .copied()
        .map(ModuleInstanceKey::from_u128)
        .collect()
}

fn owner_label(name: &StableName) -> String {
    hex(&name.node.to_be_bytes())
}

fn node_label(node: NodeKey) -> String {
    hex(&node.as_u128().to_be_bytes())
}

fn check_provenance<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    installed: &Installed,
    episodes: Vec<CheckedEpisode<D>>,
) -> Result<RestoredProvenance<D>, RestoreFailure<D>> {
    let indexed = index_records(&artifact.provenance.records)?;
    reject_inapplicable(&indexed)?;
    reject_record_revisions(artifact, &indexed)?;
    if let Lifecycle::Ready(ready) = &artifact.lifecycle {
        for record in indexed.values() {
            if let Some(time) = record.stimulus_time {
                let stamp = (time, record.order.unwrap_or(0));
                if stamp > (ready.time, ready.order)
                    || record.time.is_some_and(|basis| basis < time)
                    || matches!(record.kind, RecordKind::Initialization) && stamp.1 != 0
                {
                    return Err(fail_graph(
                        GraphFault::Conflict,
                        &hex(&record.digest),
                        record_kind_name(&record.kind),
                        "reaction stamp",
                    ));
                }
                if !matches!(
                    record.kind,
                    RecordKind::PendingPulse { .. }
                        | RecordKind::PendingTransport { .. }
                        | RecordKind::PendingInertial { .. }
                        | RecordKind::PendingPeriodic { .. }
                ) && record.time != Some(time)
                {
                    return Err(fail_graph(
                        GraphFault::Conflict,
                        &hex(&record.digest),
                        record_kind_name(&record.kind),
                        "occurrence time",
                    ));
                }
            }
        }
    }
    reject_roles(&indexed)?;
    reject_unresolved_subjects(compiled, &indexed)?;
    reject_missing_predecessors(&indexed)?;
    let graph = predecessor_graph(&indexed);
    if kahn_order(&graph).is_none() {
        return Err(fail_graph(GraphFault::Cycle, "", "record", "cycle"));
    }
    let roots = all_roots(&artifact.provenance);
    let reached = reachable(&roots, &indexed)
        .ok_or_else(|| fail_graph(GraphFault::Closure, "", "record", "root"))?;
    if reached.len() != indexed.len() {
        return Err(fail_graph(GraphFault::Closure, "", "record", "closure"));
    }
    if let Lifecycle::Ready(ready) = &artifact.lifecycle {
        if let Boundary::Checkpoint(name) = ready.boundary {
            return Err(fail_graph(GraphFault::Checkpoint, "", "boundary", name));
        }
    }
    if matches!(artifact.lifecycle, Lifecycle::Awaiting) {
        return Ok(empty_provenance());
    }
    restore_ready_provenance(compiled, artifact, installed, episodes, &indexed, &graph)
}

fn index_records<D>(
    records: &[ParsedRecord],
) -> Result<BTreeMap<[u8; 32], &ParsedRecord>, RestoreFailure<D>> {
    let mut indexed: BTreeMap<[u8; 32], &ParsedRecord> = BTreeMap::new();
    for record in records {
        match indexed.get(&record.digest) {
            Some(existing) if existing.bytes != record.bytes => {
                return Err(fail_collision(
                    &record.digest,
                    &existing.bytes,
                    &record.bytes,
                ));
            }
            Some(_) => {
                return Err(fail_graph(
                    GraphFault::Conflict,
                    &hex(&record.digest),
                    record_kind_name(&record.kind),
                    "duplicate",
                ));
            }
            None => {
                indexed.insert(record.digest, record);
            }
        }
    }
    Ok(indexed)
}

fn reject_inapplicable<D>(
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
) -> Result<(), RestoreFailure<D>> {
    for record in records.values() {
        if record.inapplicable {
            return Err(fail_graph(
                GraphFault::Digest,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "inapplicable",
            ));
        }
    }
    Ok(())
}

fn reject_record_revisions<D>(
    artifact: &Artifact,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
) -> Result<(), RestoreFailure<D>> {
    for record in records.values() {
        if record
            .revision
            .is_some_and(|revision| revision > artifact.revision.value())
        {
            return Err(fail_revision(
                artifact.revision.value(),
                record.revision.unwrap_or(0),
            ));
        }
    }
    Ok(())
}

fn reject_roles<D>(records: &BTreeMap<[u8; 32], &ParsedRecord>) -> Result<(), RestoreFailure<D>> {
    for record in records.values() {
        for predecessor in &record.predecessors {
            let allowed = match &predecessor.role {
                PredecessorRole::Supporter => true,
                PredecessorRole::Contribution => matches!(
                    record.kind,
                    RecordKind::PulseDerived { .. } | RecordKind::PulseControlled { .. }
                ),
                PredecessorRole::Other(role) => {
                    return Err(fail_graph(
                        GraphFault::Role,
                        &hex(&record.digest),
                        record_kind_name(&record.kind),
                        role,
                    ));
                }
            };
            if !allowed {
                return Err(fail_graph(
                    GraphFault::Role,
                    &hex(&record.digest),
                    record_kind_name(&record.kind),
                    "role",
                ));
            }
        }
    }
    Ok(())
}

fn reject_unresolved_subjects<D>(
    compiled: &CompiledNetwork<D>,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
) -> Result<(), RestoreFailure<D>> {
    for record in records.values() {
        if !subject_resolves(compiled, record) {
            return Err(fail_graph(
                GraphFault::Subject,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "subject",
            ));
        }
        for predecessor in &record.predecessors {
            if let Some(contribution) = &predecessor.contribution {
                if resolve_port(compiled, &contribution.port).is_none() {
                    return Err(fail_graph(
                        GraphFault::Subject,
                        &hex(&record.digest),
                        record_kind_name(&record.kind),
                        "port",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn subject_resolves<D>(compiled: &CompiledNetwork<D>, record: &ParsedRecord) -> bool {
    match &record.kind {
        RecordKind::Initialization | RecordKind::Ready => true,
        RecordKind::TopologyChange { .. } | RecordKind::Checkpoint { .. } => true,
        RecordKind::ExternalObservation { .. } => record
            .subject
            .as_ref()
            .and_then(level_input_key)
            .is_some_and(|key| compiled.contains_external_level_input(key)),
        RecordKind::ExternalPulse { .. } => record
            .subject
            .as_ref()
            .and_then(pulse_input_key)
            .is_some_and(|key| compiled.contains_external_pulse_input(key)),
        RecordKind::PendingPulse { .. }
        | RecordKind::PendingTransport { .. }
        | RecordKind::PendingInertial { .. }
        | RecordKind::PendingPeriodic { .. } => record
            .subject
            .as_ref()
            .and_then(|subject| node_key(compiled, subject))
            .is_some(),
        RecordKind::Migration { .. }
        | RecordKind::Derived
        | RecordKind::PulseDerived { .. }
        | RecordKind::PulseControlled { .. } => match &record.subject {
            Some(ParsedSubject::Node(name)) => compiled
                .resolve_stable_node(
                    &instance_keys(&name.instances),
                    NodeKey::from_u128(name.node),
                )
                .is_some(),
            Some(ParsedSubject::ExternalOutput(key)) => {
                compiled.contains_external_level_output(ExternalOutputKey::from_u128(*key))
            }
            Some(ParsedSubject::ExternalPulseOutput(key)) => {
                compiled.contains_external_pulse_output(ExternalOutputKey::from_u128(*key))
            }
            _ => false,
        },
    }
}

fn level_input_key(subject: &ParsedSubject) -> Option<ExternalInputKey<Level>> {
    match subject {
        ParsedSubject::ExternalInput(key) => Some(ExternalInputKey::from_u128(*key)),
        _ => None,
    }
}

fn pulse_input_key(subject: &ParsedSubject) -> Option<ExternalInputKey<Pulse>> {
    match subject {
        ParsedSubject::ExternalPulseInput(key) => Some(ExternalInputKey::from_u128(*key)),
        _ => None,
    }
}

fn node_key<D>(compiled: &CompiledNetwork<D>, subject: &ParsedSubject) -> Option<NodeKey> {
    let ParsedSubject::Node(name) = subject else {
        return None;
    };
    compiled.resolve_stable_node(
        &instance_keys(&name.instances),
        NodeKey::from_u128(name.node),
    )
}

fn resolve_port<D>(compiled: &CompiledNetwork<D>, port: &PortName) -> Option<PulsePortSubject> {
    match port {
        PortName::Top(port) => compiled.resolve_pulse_port(&[], *port),
        PortName::Module { instances, port } => {
            compiled.resolve_pulse_port(&instance_keys(instances), *port)
        }
    }
}

fn reject_missing_predecessors<D>(
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
) -> Result<(), RestoreFailure<D>> {
    for record in records.values() {
        for predecessor in &record.predecessors {
            if !records.contains_key(&predecessor.digest) {
                return Err(fail_graph(
                    GraphFault::Missing,
                    &hex(&record.digest),
                    record_kind_name(&record.kind),
                    &hex(&predecessor.digest),
                ));
            }
        }
    }
    Ok(())
}

fn predecessor_graph(
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
) -> BTreeMap<[u8; 32], Vec<[u8; 32]>> {
    records
        .iter()
        .map(|(digest, record)| {
            (
                *digest,
                record
                    .predecessors
                    .iter()
                    .map(|predecessor| predecessor.digest)
                    .collect(),
            )
        })
        .collect()
}

fn all_roots(provenance: &ProvenanceSection) -> Vec<[u8; 32]> {
    provenance
        .episodes
        .iter()
        .chain(provenance.external_inputs.iter())
        .chain(provenance.output_baselines.iter())
        .chain(provenance.pending_events.iter())
        .chain(provenance.state.iter())
        .copied()
        .collect()
}

fn reachable(
    roots: &[[u8; 32]],
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
) -> Option<BTreeSet<[u8; 32]>> {
    let mut reached = BTreeSet::new();
    let mut pending = Vec::new();
    for root in roots {
        if !records.contains_key(root) {
            return None;
        }
        pending.push(*root);
    }
    while let Some(digest) = pending.pop() {
        if !reached.insert(digest) {
            continue;
        }
        let record = records.get(&digest)?;
        for predecessor in &record.predecessors {
            pending.push(predecessor.digest);
        }
    }
    Some(reached)
}

fn empty_provenance<D>() -> RestoredProvenance<D> {
    RestoredProvenance {
        view: None,
        episodes: BTreeMap::new(),
        inputs: BTreeMap::new(),
        outputs: BTreeMap::new(),
        edges: BTreeMap::new(),
        toggles: BTreeMap::new(),
        establishments: BTreeMap::new(),
        transports: BTreeMap::new(),
        inertial_cancels: BTreeMap::new(),
        anchors: BTreeMap::new(),
        periodic_cancels: BTreeMap::new(),
        pending: BTreeMap::new(),
        operation: None,
    }
}

fn restore_ready_provenance<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    installed: &Installed,
    episodes: Vec<CheckedEpisode<D>>,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    graph: &BTreeMap<[u8; 32], Vec<[u8; 32]>>,
) -> Result<RestoredProvenance<D>, RestoreFailure<D>> {
    let machine_roots = machine_root_list(&artifact.provenance);
    let machine_members = reachable(&machine_roots, records)
        .ok_or_else(|| fail_graph(GraphFault::Closure, "", "record", "machine"))?;
    let inspection_fact = cbor_decode::encode(&Value::Array(vec![
        Value::Text("restored_execution_state".to_owned()),
        Value::Bytes(artifact.execution.as_bytes().to_vec()),
    ]));
    let (view, ordinals) = build_view(
        compiled,
        records,
        graph,
        &machine_members,
        installed,
        Some(&inspection_fact),
    )?;
    let scope = view.scope();
    let inspection_checkpoint = (view.len() > ordinals.len()).then_some(CauseRef::from_parts(
        scope,
        u32::try_from(ordinals.len()).unwrap_or(u32::MAX),
    ));
    let mut restored = empty_provenance();
    restored.view = Some(view);
    assign_inputs(
        installed,
        artifact,
        records,
        &ordinals,
        scope,
        &mut restored,
    )?;
    assign_baselines(
        installed,
        artifact,
        records,
        &ordinals,
        scope,
        &mut restored,
    )?;
    assign_pending(
        compiled,
        installed,
        artifact,
        records,
        &ordinals,
        scope,
        &mut restored,
    )?;
    assign_state(compiled, artifact, records, &ordinals, scope, &mut restored)?;
    require_edge_observation_causes(compiled, &restored)?;
    restored.operation =
        latest_transaction(records, &machine_members, &ordinals, scope)?.or(inspection_checkpoint);
    restored.episodes = episode_views(compiled, artifact, episodes, records, graph)?;
    Ok(restored)
}

fn machine_root_list(provenance: &ProvenanceSection) -> Vec<[u8; 32]> {
    provenance
        .external_inputs
        .iter()
        .chain(provenance.output_baselines.iter())
        .chain(provenance.pending_events.iter())
        .chain(provenance.state.iter())
        .copied()
        .collect()
}

type BuiltView<D> = (ProvenanceView<D>, BTreeMap<[u8; 32], u32>);

fn build_view<D>(
    compiled: &CompiledNetwork<D>,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    graph: &BTreeMap<[u8; 32], Vec<[u8; 32]>>,
    members: &BTreeSet<[u8; 32]>,
    installed: &Installed,
    inspection_checkpoint: Option<&[u8]>,
) -> Result<BuiltView<D>, RestoreFailure<D>> {
    let order = match kahn_order(&subgraph(graph, members)) {
        Some(order) => order,
        None => return Err(fail_graph(GraphFault::Cycle, "", "record", "view")),
    };
    let ordinals = order
        .iter()
        .enumerate()
        .map(|(index, digest)| (*digest, u32::try_from(index).unwrap_or(u32::MAX)))
        .collect::<BTreeMap<_, _>>();
    let mut built = Vec::with_capacity(order.len());
    for digest in &order {
        let record = records
            .get(digest)
            .ok_or_else(|| fail_graph(GraphFault::Digest, &hex(digest), "record", "missing"))?;
        built.push(materialize_record(compiled, record, &ordinals, installed)?);
    }
    // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "events-episodes-and-provenance"
    // A valid ready snapshot can retain no transaction ancestry; its checked state
    // still provides an inspection cause without changing any persisted semantic root.
    if !built.iter().any(|record| {
        matches!(
            record,
            ProvenanceRecord::InitializationTransaction { .. }
                | ProvenanceRecord::ReadyTransaction { .. }
                | ProvenanceRecord::TopologyChange { .. }
        )
    }) {
        if let Some(fact) = inspection_checkpoint {
            built.push(ProvenanceRecord::Checkpoint {
                fact: fact.to_vec(),
                supporters: Vec::new(),
            });
        }
    }
    let view = ProvenanceView::restored(compiled, built);
    Ok((view, ordinals))
}

fn subgraph(
    graph: &BTreeMap<[u8; 32], Vec<[u8; 32]>>,
    members: &BTreeSet<[u8; 32]>,
) -> BTreeMap<[u8; 32], Vec<[u8; 32]>> {
    members
        .iter()
        .map(|digest| {
            let predecessors = graph
                .get(digest)
                .into_iter()
                .flatten()
                .copied()
                .filter(|predecessor| members.contains(predecessor))
                .collect();
            (*digest, predecessors)
        })
        .collect()
}

fn materialize_record<D>(
    compiled: &CompiledNetwork<D>,
    record: &ParsedRecord,
    ordinals: &BTreeMap<[u8; 32], u32>,
    installed: &Installed,
) -> Result<ProvenanceRecord<D>, RestoreFailure<D>> {
    let supporters = supporter_causes(record, ordinals)?;
    let contributions = contribution_causes(compiled, record, ordinals)?;
    match &record.kind {
        RecordKind::TopologyChange { base, target } => Ok(ProvenanceRecord::TopologyChange {
            at: required_stamp(record)?,
            revision: required_revision(record)?,
            base: NetworkFingerprint::from_digest(*base),
            target: NetworkFingerprint::from_digest(*target),
            supporters,
        }),
        RecordKind::Migration { rule } => Ok(ProvenanceRecord::Migration {
            subject: provenance_subject(compiled, record)?,
            rule: rule.clone(),
            supporters,
        }),
        RecordKind::Checkpoint { fact } => Ok(ProvenanceRecord::Checkpoint {
            fact: fact.clone(),
            supporters,
        }),
        RecordKind::Initialization => Ok(ProvenanceRecord::InitializationTransaction {
            at: required_stamp(record)?,
            revision: required_revision(record)?,
        }),
        RecordKind::Ready => Ok(ProvenanceRecord::ReadyTransaction {
            at: required_stamp(record)?,
            revision: required_revision(record)?,
        }),
        RecordKind::ExternalObservation { value } => Ok(ProvenanceRecord::ExternalObservation {
            stamp: required_stamp(record)?,
            input: required_level_input(record)?,
            value: *value,
        }),
        RecordKind::ExternalPulse { count } => Ok(ProvenanceRecord::ExternalPulseObservation {
            stamp: required_stamp(record)?,
            input: required_pulse_input(record)?,
            count: PulseCount::new(*count),
        }),
        RecordKind::PendingPulse { count } => {
            let (event, owner, origin, deadline, revision) =
                pending_facts(compiled, record, installed)?;
            Ok(ProvenanceRecord::PendingPulseDelay {
                stimulus: required_stamp(record)?,
                event,
                owner,
                origin,
                deadline,
                count: PulseCount::new(*count),
                revision,
                supporters,
            })
        }
        RecordKind::PendingTransport { target } => {
            let (event, owner, origin, deadline, revision) =
                pending_facts(compiled, record, installed)?;
            Ok(ProvenanceRecord::PendingTransportDelay {
                stimulus: required_stamp(record)?,
                event,
                owner,
                origin,
                deadline,
                target: *target,
                revision,
                supporters,
            })
        }
        RecordKind::PendingInertial { target } => {
            let (event, owner, origin, deadline, revision) =
                pending_facts(compiled, record, installed)?;
            Ok(ProvenanceRecord::PendingInertialDelay {
                stimulus: required_stamp(record)?,
                event,
                owner,
                origin,
                deadline,
                target: *target,
                revision,
                supporters,
            })
        }
        RecordKind::PendingPeriodic {
            anchor,
            ordinal,
            first_emission,
            reenable_phase,
        } => {
            let (event, owner, origin, deadline, revision) =
                pending_facts(compiled, record, installed)?;
            let node = match &owner {
                NodeSubject::Node(node) => *node,
                NodeSubject::Qualified(_) => flat_from_subject(compiled, record)?,
            };
            let periodic_node = flat_from_subject(compiled, record)?;
            let _ = node;
            agree_periodic(
                compiled,
                periodic_node,
                *first_emission,
                *reenable_phase,
                record,
            )?;
            Ok(ProvenanceRecord::PendingPeriodicBoundary {
                stimulus: required_stamp(record)?,
                event,
                owner,
                origin,
                deadline,
                anchor: Time::from_ticks(*anchor),
                ordinal: *ordinal,
                first_emission: *first_emission,
                reenable_phase: *reenable_phase,
                revision,
                supporters,
            })
        }
        RecordKind::Derived => Ok(ProvenanceRecord::Derived {
            subject: provenance_subject(compiled, record)?,
            supporters,
        }),
        RecordKind::PulseDerived { result } => Ok(ProvenanceRecord::PulseDerived {
            subject: provenance_subject(compiled, record)?,
            contributions,
            result: PulseCount::new(*result),
            supporters,
        }),
        RecordKind::PulseControlled { result } => Ok(ProvenanceRecord::PulseControlledLevel {
            subject: provenance_subject(compiled, record)?,
            contributions,
            result: *result,
            supporters,
        }),
    }
}

fn supporter_causes<D>(
    record: &ParsedRecord,
    ordinals: &BTreeMap<[u8; 32], u32>,
) -> Result<Vec<CauseRef>, RestoreFailure<D>> {
    let mut supporters = Vec::new();
    for predecessor in &record.predecessors {
        if matches!(predecessor.role, PredecessorRole::Supporter) {
            supporters.push(unfinalized_cause(ordinals, predecessor.digest)?);
        }
    }
    Ok(supporters)
}

fn contribution_causes<D>(
    compiled: &CompiledNetwork<D>,
    record: &ParsedRecord,
    ordinals: &BTreeMap<[u8; 32], u32>,
) -> Result<Vec<PulseContribution>, RestoreFailure<D>> {
    let mut contributions = Vec::new();
    for predecessor in &record.predecessors {
        if !matches!(predecessor.role, PredecessorRole::Contribution) {
            continue;
        }
        let Some(contribution) = &predecessor.contribution else {
            return Err(fail_graph(
                GraphFault::Digest,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "contribution",
            ));
        };
        let Some(port) = resolve_port(compiled, &contribution.port) else {
            return Err(fail_graph(
                GraphFault::Subject,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "port",
            ));
        };
        contributions.push(PulseContribution::restored(
            port,
            PulseCount::new(contribution.count),
            unfinalized_cause(ordinals, predecessor.digest)?,
        ));
    }
    Ok(contributions)
}

fn unfinalized_cause<D>(
    ordinals: &BTreeMap<[u8; 32], u32>,
    digest: [u8; 32],
) -> Result<CauseRef, RestoreFailure<D>> {
    match ordinals.get(&digest).copied() {
        Some(ordinal) => Ok(CauseRef::from_parts([0; 32], ordinal)),
        None => Err(fail_graph(
            GraphFault::Digest,
            &hex(&digest),
            "record",
            "ordinal",
        )),
    }
}

fn scoped_cause<D>(
    ordinals: &BTreeMap<[u8; 32], u32>,
    digest: [u8; 32],
    scope: [u8; 32],
) -> Result<CauseRef, RestoreFailure<D>> {
    match ordinals.get(&digest).copied() {
        Some(ordinal) => Ok(CauseRef::from_parts(scope, ordinal)),
        None => Err(fail_graph(
            GraphFault::Digest,
            &hex(&digest),
            "record",
            "scope",
        )),
    }
}

fn required_stamp<D>(record: &ParsedRecord) -> Result<crate::ReactionStamp<D>, RestoreFailure<D>> {
    let time = record.stimulus_time.ok_or_else(|| {
        fail_graph(
            GraphFault::Conflict,
            &hex(&record.digest),
            "record",
            "stimulus_time",
        )
    })?;
    let order = record.order.ok_or_else(|| {
        fail_graph(
            GraphFault::Conflict,
            &hex(&record.digest),
            "record",
            "reaction_order",
        )
    })?;
    Ok(crate::ReactionStamp::from_parts(
        Time::from_ticks(time),
        order,
    ))
}

fn required_time<D>(record: &ParsedRecord) -> Result<u64, RestoreFailure<D>> {
    record.time.ok_or_else(|| {
        fail_graph(
            GraphFault::Digest,
            &hex(&record.digest),
            record_kind_name(&record.kind),
            "time",
        )
    })
}

fn required_revision<D>(record: &ParsedRecord) -> Result<NetworkRevision, RestoreFailure<D>> {
    record
        .revision
        .map(NetworkRevision::from_value)
        .ok_or_else(|| {
            fail_graph(
                GraphFault::Digest,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "revision",
            )
        })
}

fn required_level_input<D>(
    record: &ParsedRecord,
) -> Result<ExternalInputKey<Level>, RestoreFailure<D>> {
    record
        .subject
        .as_ref()
        .and_then(level_input_key)
        .ok_or_else(|| {
            fail_graph(
                GraphFault::Subject,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "input",
            )
        })
}

fn required_pulse_input<D>(
    record: &ParsedRecord,
) -> Result<ExternalInputKey<Pulse>, RestoreFailure<D>> {
    record
        .subject
        .as_ref()
        .and_then(pulse_input_key)
        .ok_or_else(|| {
            fail_graph(
                GraphFault::Subject,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "pulse",
            )
        })
}

type PendingIdentity<D> = (
    PendingEventKey,
    NodeSubject,
    Time<D>,
    Time<D>,
    NetworkRevision,
);

fn pending_facts<D>(
    compiled: &CompiledNetwork<D>,
    record: &ParsedRecord,
    installed: &Installed,
) -> Result<PendingIdentity<D>, RestoreFailure<D>> {
    let node = flat_from_subject(compiled, record)?;
    let owner = compiled.node_subject(node);
    let origin = required_time(record)?;
    let revision = required_revision(record)?;
    let calendar = installed
        .pending
        .iter()
        .find(|event| event.cause == record.digest);
    if let Some(event) = calendar {
        agree_pending(compiled, event, record, node, origin, revision.value())?;
        Ok((
            PendingEventKey::from_serial(event.key),
            owner,
            Time::from_ticks(origin),
            Time::from_ticks(event.deadline),
            revision,
        ))
    } else {
        Ok((
            PendingEventKey::from_serial(0),
            owner,
            Time::from_ticks(origin),
            Time::from_ticks(origin),
            revision,
        ))
    }
}

fn agree_pending<D>(
    compiled: &CompiledNetwork<D>,
    event: &ResolvedPending,
    record: &ParsedRecord,
    node: NodeKey,
    origin: u64,
    revision: u64,
) -> Result<(), RestoreFailure<D>> {
    let kind_ok = match (&record.kind, event.kind) {
        (RecordKind::PendingPulse { count }, PendingKind::Pulse { count: event_count }) => {
            *count == event_count
        }
        (
            RecordKind::PendingTransport { target },
            PendingKind::Transport {
                target: event_target,
                ..
            },
        )
        | (
            RecordKind::PendingInertial { target },
            PendingKind::Inertial {
                target: event_target,
                ..
            },
        ) => *target == event_target,
        (
            RecordKind::PendingPeriodic {
                anchor, ordinal, ..
            },
            PendingKind::Periodic {
                anchor: event_anchor,
                ordinal: event_ordinal,
            },
        ) => *anchor == event_anchor && *ordinal == event_ordinal,
        _ => false,
    };
    if event.node != node || event.origin != origin || event.revision != revision || !kind_ok {
        return Err(fail_pending(
            Some(event.key),
            &node_label(node),
            "provenance",
        ));
    }
    if let RecordKind::PendingPeriodic {
        first_emission,
        reenable_phase,
        ..
    } = &record.kind
    {
        agree_periodic(compiled, node, *first_emission, *reenable_phase, record)?;
    }
    let _ = compiled;
    Ok(())
}

fn agree_periodic<D>(
    compiled: &CompiledNetwork<D>,
    node: NodeKey,
    first_emission: FirstEmissionPolicy,
    reenable_phase: ReenablePhasePolicy,
    record: &ParsedRecord,
) -> Result<(), RestoreFailure<D>> {
    match compiled.periodic(node) {
        Some((_, compiled_first, compiled_reenable, _))
            if compiled_first == first_emission && compiled_reenable == reenable_phase =>
        {
            Ok(())
        }
        _ => Err(fail_pending(None, &hex(&record.digest), "periodic")),
    }
}

fn flat_from_subject<D>(
    compiled: &CompiledNetwork<D>,
    record: &ParsedRecord,
) -> Result<NodeKey, RestoreFailure<D>> {
    record
        .subject
        .as_ref()
        .and_then(|subject| node_key(compiled, subject))
        .ok_or_else(|| {
            fail_graph(
                GraphFault::Subject,
                &hex(&record.digest),
                record_kind_name(&record.kind),
                "node",
            )
        })
}

fn provenance_subject<D>(
    compiled: &CompiledNetwork<D>,
    record: &ParsedRecord,
) -> Result<ProvenanceSubject, RestoreFailure<D>> {
    match record.subject.as_ref() {
        Some(ParsedSubject::Node(name)) => {
            let node = compiled
                .resolve_stable_node(
                    &instance_keys(&name.instances),
                    NodeKey::from_u128(name.node),
                )
                .ok_or_else(|| {
                    fail_graph(
                        GraphFault::Subject,
                        &hex(&record.digest),
                        record_kind_name(&record.kind),
                        "node",
                    )
                })?;
            Ok(match compiled.node_subject(node) {
                NodeSubject::Node(node) => ProvenanceSubject::Node(node),
                NodeSubject::Qualified(node) => ProvenanceSubject::QualifiedNode(node),
            })
        }
        Some(ParsedSubject::ExternalOutput(key)) => Ok(ProvenanceSubject::ExternalOutput(
            ExternalOutputKey::from_u128(*key),
        )),
        Some(ParsedSubject::ExternalPulseOutput(key)) => Ok(
            ProvenanceSubject::PulseExternalOutput(ExternalOutputKey::from_u128(*key)),
        ),
        _ => Err(fail_graph(
            GraphFault::Subject,
            &hex(&record.digest),
            record_kind_name(&record.kind),
            "subject",
        )),
    }
}

fn assign_inputs<D>(
    installed: &Installed,
    artifact: &Artifact,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    ordinals: &BTreeMap<[u8; 32], u32>,
    scope: [u8; 32],
    restored: &mut RestoredProvenance<D>,
) -> Result<(), RestoreFailure<D>> {
    let mut assigned = BTreeMap::new();
    for digest in &artifact.provenance.external_inputs {
        let record = records.get(digest).ok_or_else(|| {
            fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                "external_observation",
                "root",
            )
        })?;
        let RecordKind::ExternalObservation { value } = &record.kind else {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                record_kind_name(&record.kind),
                "input",
            ));
        };
        let input = required_level_input(record)?;
        if installed.levels.get(&input) != Some(value) || assigned.insert(input, *digest).is_some()
        {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                "external_observation",
                "level",
            ));
        }
    }
    if assigned.len() != installed.levels.len() {
        return Err(fail_graph(
            GraphFault::Conflict,
            "",
            "external_observation",
            "coverage",
        ));
    }
    for (input, digest) in assigned {
        restored
            .inputs
            .insert(input, scoped_cause(ordinals, digest, scope)?);
    }
    Ok(())
}

fn assign_baselines<D>(
    installed: &Installed,
    artifact: &Artifact,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    ordinals: &BTreeMap<[u8; 32], u32>,
    scope: [u8; 32],
    restored: &mut RestoredProvenance<D>,
) -> Result<(), RestoreFailure<D>> {
    let mut assigned = BTreeSet::new();
    for digest in &artifact.provenance.output_baselines {
        let record = records
            .get(digest)
            .ok_or_else(|| fail_graph(GraphFault::Conflict, &hex(digest), "derived", "baseline"))?;
        let key = match &record.subject {
            Some(ParsedSubject::ExternalOutput(key)) => ExternalOutputKey::<Level>::from_u128(*key),
            _ => {
                return Err(fail_graph(
                    GraphFault::Conflict,
                    &hex(digest),
                    record_kind_name(&record.kind),
                    "baseline",
                ));
            }
        };
        let Some((level, cause)) = installed.baselines.get(&key) else {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                "derived",
                "baseline",
            ));
        };
        if cause != digest {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                "derived",
                "cause",
            ));
        }
        let _ = level;
        if !assigned.insert(key) {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                "derived",
                "duplicate",
            ));
        }
        restored
            .outputs
            .insert(key, scoped_cause(ordinals, *digest, scope)?);
    }
    if assigned.len() != installed.baselines.len() {
        return Err(fail_graph(GraphFault::Conflict, "", "derived", "coverage"));
    }
    Ok(())
}

fn assign_pending<D>(
    _compiled: &CompiledNetwork<D>,
    installed: &Installed,
    artifact: &Artifact,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    ordinals: &BTreeMap<[u8; 32], u32>,
    scope: [u8; 32],
    restored: &mut RestoredProvenance<D>,
) -> Result<(), RestoreFailure<D>> {
    let mut causes = BTreeSet::new();
    for event in &installed.pending {
        if !causes.insert(event.cause) {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(&event.cause),
                event.node_kind,
                "pending",
            ));
        }
        let record = records
            .get(&event.cause)
            .ok_or_else(|| fail_pending(Some(event.key), &node_label(event.node), "cause"))?;
        if !pending_record_kind(&record.kind, event.node_kind)
            || record.stimulus_time != Some(event.stimulus.0)
            || record.order != Some(event.stimulus.1)
        {
            return Err(fail_pending(
                Some(event.key),
                &node_label(event.node),
                "record",
            ));
        }
        let cause = scoped_cause(ordinals, event.cause, scope)?;
        let pending = pending_event(*event, cause, record)?;
        restored
            .pending
            .entry(Time::from_ticks(event.deadline))
            .or_default()
            .push(pending);
    }
    let roots: BTreeSet<[u8; 32]> = artifact.provenance.pending_events.iter().copied().collect();
    if causes != roots {
        return Err(fail_graph(GraphFault::Conflict, "", "pending", "roots"));
    }
    Ok(())
}

fn pending_record_kind(kind: &RecordKind, node_kind: &str) -> bool {
    matches!(
        (kind, node_kind),
        (RecordKind::PendingPulse { .. }, "pulse_delay")
            | (RecordKind::PendingTransport { .. }, "transport_delay")
            | (RecordKind::PendingInertial { .. }, "inertial_delay")
            | (RecordKind::PendingPeriodic { .. }, "periodic")
    )
}

fn pending_event<D>(
    event: ResolvedPending,
    cause: CauseRef,
    record: &ParsedRecord,
) -> Result<PendingEvent<D>, RestoreFailure<D>> {
    let key = PendingEventKey::from_serial(event.key);
    let origin = Time::from_ticks(event.origin);
    let deadline = Time::from_ticks(event.deadline);
    let revision = NetworkRevision::from_value(event.revision);
    match (event.kind, &record.kind) {
        (PendingKind::Pulse { count }, RecordKind::PendingPulse { .. }) => {
            Ok(PendingEvent::PulseDelay(PendingPulseDelay {
                stimulus: crate::ReactionStamp::from_parts(
                    Time::from_ticks(event.stimulus.0),
                    event.stimulus.1,
                ),
                key,
                node: event.node,
                origin,
                deadline,
                count: PulseCount::new(count),
                revision,
                cause,
            }))
        }
        (PendingKind::Transport { target, .. }, RecordKind::PendingTransport { .. }) => {
            Ok(PendingEvent::TransportDelay(PendingTransportDelay {
                stimulus: crate::ReactionStamp::from_parts(
                    Time::from_ticks(event.stimulus.0),
                    event.stimulus.1,
                ),
                key,
                node: event.node,
                origin,
                deadline,
                target,
                revision,
                cause,
            }))
        }
        (PendingKind::Inertial { target, .. }, RecordKind::PendingInertial { .. }) => {
            Ok(PendingEvent::Inertial(PendingInertialDelay {
                stimulus: crate::ReactionStamp::from_parts(
                    Time::from_ticks(event.stimulus.0),
                    event.stimulus.1,
                ),
                key,
                node: event.node,
                origin,
                deadline,
                target,
                revision,
                cause,
            }))
        }
        (
            PendingKind::Periodic { anchor, ordinal },
            RecordKind::PendingPeriodic {
                first_emission,
                reenable_phase,
                ..
            },
        ) => Ok(PendingEvent::Periodic(PendingPeriodicBoundary {
            stimulus: crate::ReactionStamp::from_parts(
                Time::from_ticks(event.stimulus.0),
                event.stimulus.1,
            ),
            key,
            node: event.node,
            origin,
            deadline,
            anchor: Time::from_ticks(anchor),
            ordinal,
            first_emission: *first_emission,
            reenable_phase: *reenable_phase,
            revision,
            cause,
        })),
        _ => Err(fail_pending(
            Some(event.key),
            &node_label(event.node),
            "kind",
        )),
    }
}

fn assign_state<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    ordinals: &BTreeMap<[u8; 32], u32>,
    scope: [u8; 32],
    restored: &mut RestoredProvenance<D>,
) -> Result<(), RestoreFailure<D>> {
    let mut grouped: BTreeMap<NodeKey, Vec<[u8; 32]>> = BTreeMap::new();
    for digest in &artifact.provenance.state {
        let record = records
            .get(digest)
            .ok_or_else(|| fail_graph(GraphFault::Conflict, &hex(digest), "derived", "state"))?;
        let Some(node) = record
            .subject
            .as_ref()
            .and_then(|subject| node_key(compiled, subject))
        else {
            return Err(fail_graph(
                GraphFault::Conflict,
                &hex(digest),
                record_kind_name(&record.kind),
                "state",
            ));
        };
        grouped.entry(node).or_default().push(*digest);
    }
    for (node, digests) in grouped {
        assign_node_roots(compiled, records, ordinals, scope, restored, node, &digests)?;
    }
    Ok(())
}

fn require_edge_observation_causes<D>(
    compiled: &CompiledNetwork<D>,
    restored: &RestoredProvenance<D>,
) -> Result<(), RestoreFailure<D>> {
    // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "events-episodes-and-provenance"
    // A ready edge retains the cause of its previous observation. The next
    // transaction reads that cause from the restored machine.
    for node in compiled.snapshot_nodes() {
        if !matches!(node.family, SnapshotNodeFamily::Edge { .. }) {
            continue;
        }
        if !restored.edges.contains_key(&node.flat) {
            return Err(fail_graph(
                GraphFault::Closure,
                &node_label(node.flat),
                "derived",
                "edge",
            ));
        }
    }
    Ok(())
}

fn assign_node_roots<D>(
    compiled: &CompiledNetwork<D>,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    ordinals: &BTreeMap<[u8; 32], u32>,
    scope: [u8; 32],
    restored: &mut RestoredProvenance<D>,
    node: NodeKey,
    digests: &[[u8; 32]],
) -> Result<(), RestoreFailure<D>> {
    let Some(family) = cause_family(compiled, node) else {
        return Err(fail_graph(
            GraphFault::Conflict,
            &node_label(node),
            "state",
            "family",
        ));
    };
    if family == CauseFamily::Pulse
        || digests.len() > 2
        || (digests.len() == 2 && !family_has_secondary(family))
    {
        return Err(fail_graph(
            GraphFault::Conflict,
            &node_label(node),
            "state",
            "roots",
        ));
    }
    if digests.is_empty() {
        return Ok(());
    }
    if digests.len() == 1 {
        let cause = scoped_cause(ordinals, digests[0], scope)?;
        // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "events-episodes-and-provenance"
        // RestartPhase can leave only a cancellation root after discarding its anchor.
        if family_has_secondary(family) && is_cancellation_root(records, digests[0]) {
            insert_secondary(restored, family, node, cause)?;
        } else {
            insert_primary(restored, family, node, cause);
        }
        return Ok(());
    }
    let first_pending = is_cancellation_root(records, digests[0]);
    let second_pending = is_cancellation_root(records, digests[1]);
    let (primary, secondary) = match (first_pending, second_pending) {
        (false, true) => (digests[0], digests[1]),
        (true, false) => (digests[1], digests[0]),
        _ => {
            return Err(fail_graph(
                GraphFault::Conflict,
                &node_label(node),
                "state",
                "cancellation",
            ));
        }
    };
    insert_primary(
        restored,
        family,
        node,
        scoped_cause(ordinals, primary, scope)?,
    );
    let secondary = scoped_cause(ordinals, secondary, scope)?;
    insert_secondary(restored, family, node, secondary)
}

fn insert_secondary<D>(
    restored: &mut RestoredProvenance<D>,
    family: CauseFamily,
    node: NodeKey,
    cause: CauseRef,
) -> Result<(), RestoreFailure<D>> {
    match family {
        CauseFamily::Inertial => {
            restored.inertial_cancels.insert(node, cause);
        }
        CauseFamily::Periodic => {
            restored.periodic_cancels.insert(node, cause);
        }
        _ => {
            return Err(fail_graph(
                GraphFault::Conflict,
                &node_label(node),
                "state",
                "secondary",
            ));
        }
    }
    Ok(())
}

fn family_has_secondary(family: CauseFamily) -> bool {
    matches!(family, CauseFamily::Inertial | CauseFamily::Periodic)
}

fn is_cancellation_root(records: &BTreeMap<[u8; 32], &ParsedRecord>, digest: [u8; 32]) -> bool {
    records.get(&digest).is_some_and(|record| {
        if matches!(&record.kind, RecordKind::Migration { rule } if rule == MIGRATED_CANCELLATION_RULE) {
            return true;
        }
        // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "events-episodes-and-provenance"
        // A matured transition supports pending work and its transaction directly;
        // cancellation instead supports the canceled work and a replacement derivation.
        let mut pending = false;
        let mut transaction = false;
        for predecessor in &record.predecessors {
            if !matches!(predecessor.role, PredecessorRole::Supporter) {
                continue;
            }
            let Some(predecessor) = records.get(&predecessor.digest) else {
                continue;
            };
            pending |= is_pending_record(&predecessor.kind);
            transaction |= matches!(predecessor.kind,
                RecordKind::Initialization | RecordKind::Ready | RecordKind::TopologyChange { .. });
        }
        pending && !transaction
    })
}

fn is_pending_record(kind: &RecordKind) -> bool {
    matches!(
        kind,
        RecordKind::PendingPulse { .. }
            | RecordKind::PendingTransport { .. }
            | RecordKind::PendingInertial { .. }
            | RecordKind::PendingPeriodic { .. }
    )
}

fn cause_family<D>(compiled: &CompiledNetwork<D>, node: NodeKey) -> Option<CauseFamily> {
    let kind = node_identity(compiled, node)?;
    Some(match kind {
        "rising_edge" | "falling_edge" | "any_edge" => CauseFamily::Edge,
        "toggle" => CauseFamily::Toggle,
        "pulse_set_reset_latch" | "level_set_reset_latch" | "sample_hold" => {
            CauseFamily::Establishment
        }
        "transport_delay" => CauseFamily::Transport,
        "inertial_delay" => CauseFamily::Inertial,
        "periodic" => CauseFamily::Periodic,
        "pulse_delay" => CauseFamily::Pulse,
        _ => return None,
    })
}

fn insert_primary<D>(
    restored: &mut RestoredProvenance<D>,
    family: CauseFamily,
    node: NodeKey,
    cause: CauseRef,
) {
    match family {
        CauseFamily::Edge => {
            restored.edges.insert(node, cause);
        }
        CauseFamily::Toggle => {
            restored.toggles.insert(node, cause);
        }
        CauseFamily::Establishment => {
            restored.establishments.insert(node, cause);
        }
        CauseFamily::Transport | CauseFamily::Inertial => {
            restored.transports.insert(node, cause);
        }
        CauseFamily::Periodic => {
            restored.anchors.insert(node, cause);
        }
        CauseFamily::Pulse => {}
    }
}

fn latest_transaction<D>(
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    members: &BTreeSet<[u8; 32]>,
    ordinals: &BTreeMap<[u8; 32], u32>,
    scope: [u8; 32],
) -> Result<Option<CauseRef>, RestoreFailure<D>> {
    let mut selected: Option<((u64, u64), [u8; 32])> = None;
    for digest in members {
        let Some(record) = records.get(digest) else {
            continue;
        };
        if !matches!(
            record.kind,
            RecordKind::Initialization | RecordKind::Ready | RecordKind::TopologyChange { .. }
        ) {
            continue;
        }
        let time = (record.time.unwrap_or(0), record.order.unwrap_or(0));
        let replace = match selected {
            Some((selected_time, selected_digest)) => {
                (time, *digest) > (selected_time, selected_digest)
            }
            None => true,
        };
        if replace {
            selected = Some((time, *digest));
        }
    }
    match selected {
        Some((_, digest)) => Ok(Some(scoped_cause(ordinals, digest, scope)?)),
        None => Ok(None),
    }
}

fn episode_views<D>(
    compiled: &CompiledNetwork<D>,
    artifact: &Artifact,
    episodes: Vec<CheckedEpisode<D>>,
    records: &BTreeMap<[u8; 32], &ParsedRecord>,
    graph: &BTreeMap<[u8; 32], Vec<[u8; 32]>>,
) -> Result<BTreeMap<DiagnosticConditionKey, ActiveDiagnosticEpisode<D>>, RestoreFailure<D>> {
    let causes: BTreeSet<[u8; 32]> = episodes.iter().map(|episode| episode.cause).collect();
    let roots: BTreeSet<[u8; 32]> = artifact.provenance.episodes.iter().copied().collect();
    if causes != roots {
        return Err(fail_graph(GraphFault::Conflict, "", "episode", "roots"));
    }
    let mut restored = BTreeMap::new();
    for episode in episodes {
        let members = reachable(&[episode.cause], records).ok_or_else(|| {
            fail_graph(
                GraphFault::Closure,
                &hex(&episode.cause),
                "episode",
                "cause",
            )
        })?;
        let (view, ordinals) =
            build_view(compiled, records, graph, &members, &empty_installed(), None)?;
        let cause = scoped_cause(&ordinals, episode.cause, view.scope())?;
        restored.insert(
            episode.condition.clone(),
            ActiveDiagnosticEpisode::restored(
                episode.identity,
                episode.condition.clone(),
                episode.problem,
                crate::ReactionStamp::from_parts(
                    Time::from_ticks(episode.began),
                    episode.began_order,
                ),
                crate::ReactionStamp::from_parts(
                    Time::from_ticks(episode.changed),
                    episode.changed_order,
                ),
                cause,
                view,
            ),
        );
    }
    Ok(restored)
}

fn empty_installed() -> Installed {
    Installed {
        edges: Vec::new(),
        stored: Vec::new(),
        anchors: BTreeMap::new(),
        levels: BTreeMap::new(),
        baselines: BTreeMap::new(),
        pending: Vec::new(),
        singular: Vec::new(),
    }
}

fn check_settled<D>(
    compiled: &CompiledNetwork<D>,
    installed: &Installed,
    ready: &LifecycleReady,
) -> Result<FullEvaluation, RestoreFailure<D>> {
    let anchors = installed
        .anchors
        .iter()
        .map(|(node, phase)| (*node, phase.runtime()))
        .collect();
    let evaluation = match compiled.evaluate_reaction_with_state(
        &installed.levels,
        &BTreeMap::new(),
        &installed.edges,
        &installed.stored,
        Time::from_ticks(ready.time),
        &anchors,
    ) {
        Ok(evaluation) => evaluation,
        Err(_) => return Err(fail_settled("evaluation")),
    };
    if evaluation.proposed_edge_observations != installed.edges
        || evaluation.proposed_stored_levels != installed.stored
    {
        return Err(fail_settled("state"));
    }
    let baselines: BTreeMap<_, _> = installed
        .baselines
        .iter()
        .map(|(key, (level, _))| (*key, *level))
        .collect();
    if evaluation.external_outputs != baselines {
        return Err(fail_settled("baselines"));
    }
    if !settled_facts_match(compiled, installed, ready, &evaluation) {
        return Err(fail_settled("facts"));
    }
    Ok(evaluation)
}

fn settled_facts_match<D>(
    compiled: &CompiledNetwork<D>,
    installed: &Installed,
    ready: &LifecycleReady,
    evaluation: &FullEvaluation,
) -> bool {
    let mut expected = BTreeMap::new();
    for slot in compiled.settled_level_slots() {
        let Some(level) = evaluation
            .operation_levels
            .get(slot.operation)
            .copied()
            .flatten()
        else {
            return false;
        };
        expected.insert(encode_settled_endpoint(&slot.endpoint), level);
    }
    for (key, level) in &installed.levels {
        expected.insert(external_input_bytes(key.as_u128()), *level);
    }
    let actual: BTreeMap<_, _> = ready
        .settled
        .iter()
        .map(|fact| (fact.subject_bytes.clone(), fact.level))
        .collect();
    expected == actual
}

fn external_input_bytes(key: u128) -> Vec<u8> {
    cbor_decode::encode(&Value::Array(vec![
        Value::Text("external_input".to_owned()),
        Value::Bytes(key.to_be_bytes().to_vec()),
    ]))
}

fn publish_machine<D>(
    compiled: &CompiledNetwork<D>,
    policy: RuntimePolicy,
    artifact: &Artifact,
    installed: Installed,
    provenance: RestoredProvenance<D>,
    evaluation: Option<FullEvaluation>,
) -> Result<Machine<D>, RestoreFailure<D>> {
    let mut machine = Machine::new(compiled.clone(), policy);
    machine.store.revision = artifact.revision;
    machine.store.next_pending_event_serial = artifact.next_serial;
    machine.store.edge_observations = installed.edges;
    machine.store.stored_levels = installed.stored;
    machine.store.periodic_anchors = installed
        .anchors
        .into_iter()
        .map(|(node, phase)| (node, phase.runtime()))
        .collect();
    match (&artifact.lifecycle, evaluation) {
        (Lifecycle::Awaiting, None) => Ok(machine),
        (Lifecycle::Ready(ready), Some(evaluation)) => {
            machine.store.last_reaction = Some(crate::ReactionStamp::from_parts(
                Time::from_ticks(ready.time),
                ready.order,
            ));
            machine.store.status = MachineStatus::Ready {
                now: Time::from_ticks(ready.time),
            };
            machine.store.external_levels = installed.levels;
            machine.store.output_baselines = installed
                .baselines
                .into_iter()
                .map(|(key, (level, _))| (key, level))
                .collect();
            machine.store.settled_levels = evaluation.values;
            machine.store.operation_levels = evaluation.operation_levels;
            machine.store.provenance = provenance.view;
            machine.store.input_causes = provenance.inputs;
            machine.store.output_causes = provenance.outputs;
            machine.store.edge_observation_causes = provenance.edges;
            machine.store.toggle_inversion_causes = provenance.toggles;
            machine.store.establishment_causes = provenance.establishments;
            machine.store.transport_transition_causes = provenance.transports;
            machine.store.inertial_cancellation_causes = provenance.inertial_cancels;
            machine.store.periodic_anchor_causes = provenance.anchors;
            machine.store.periodic_cancellation_causes = provenance.periodic_cancels;
            machine.store.active_episodes = provenance.episodes;
            machine.store.pending_events = provenance.pending;
            if let Some(cause) = provenance.operation {
                // Inspection reads operation causes before the next transaction replaces them.
                // Required roots retain transaction ancestry or a checked snapshot-state checkpoint.
                machine.store.operation_causes = vec![cause; compiled.operation_count()];
            }
            Ok(machine)
        }
        _ => Err(fail_lifecycle()),
    }
}

fn check_reencoded_provenance<D>(
    machine: &Machine<D>,
    artifact: &Artifact,
) -> Result<(), RestoreFailure<D>> {
    if artifact.provenance.records.is_empty() {
        return Ok(());
    }
    let index = cause_digest_index(machine);
    for record in &artifact.provenance.records {
        match index.records().get(&record.digest) {
            Some(payload) if payload.as_ref() == &record.bytes => {}
            _ => {
                return Err(fail_graph(
                    GraphFault::Digest,
                    &hex(&record.digest),
                    "record",
                    "reencoded",
                ));
            }
        }
    }
    Ok(())
}

fn check_digests<D>(machine: &Machine<D>, artifact: &Artifact) -> Result<(), RestoreFailure<D>> {
    let execution = machine.execution_state_digest();
    if execution != artifact.execution {
        return Err(fail_execution_digest(
            &artifact.execution.as_bytes(),
            &execution.as_bytes(),
        ));
    }
    let observable = machine.observable_state_digest();
    if observable != artifact.observable {
        return Err(fail_observable_digest(
            &artifact.observable.as_bytes(),
            &observable.as_bytes(),
        ));
    }
    let snapshot = snapshot_digest_bytes(artifact.time_domain, &artifact.payload_bytes);
    if snapshot != artifact.snapshot.as_bytes() {
        return Err(fail_snapshot_digest(
            &artifact.snapshot.as_bytes(),
            &snapshot,
        ));
    }
    Ok(())
}

fn record_kind_name(kind: &RecordKind) -> &'static str {
    match kind {
        RecordKind::TopologyChange { .. } => "topology_change",
        RecordKind::Migration { .. } => "migration",
        RecordKind::Checkpoint { .. } => "checkpoint",
        RecordKind::Initialization => "initialization_transaction",
        RecordKind::Ready => "ready_transaction",
        RecordKind::ExternalObservation { .. } => "external_observation",
        RecordKind::ExternalPulse { .. } => "external_pulse_observation",
        RecordKind::PendingPulse { .. } => "pending_pulse_delay",
        RecordKind::PendingTransport { .. } => "pending_transport_delay",
        RecordKind::PendingInertial { .. } => "pending_inertial_delay",
        RecordKind::PendingPeriodic { .. } => "pending_periodic_boundary",
        RecordKind::Derived => "derived",
        RecordKind::PulseDerived { .. } => "pulse_derived",
        RecordKind::PulseControlled { .. } => "pulse_controlled_level",
    }
}

/// Orders a digest graph so every predecessor precedes its successor.
///
/// `None` means the graph contains a cycle or an edge that leaves the set.
fn kahn_order(nodes: &BTreeMap<[u8; 32], Vec<[u8; 32]>>) -> Option<Vec<[u8; 32]>> {
    let mut indegree = BTreeMap::new();
    for digest in nodes.keys() {
        indegree.insert(*digest, 0_usize);
    }
    let mut outgoing: BTreeMap<[u8; 32], Vec<[u8; 32]>> = BTreeMap::new();
    for (digest, predecessors) in nodes {
        for predecessor in predecessors {
            if predecessor == digest || !nodes.contains_key(predecessor) {
                return None;
            }
            let degree = indegree.get_mut(digest)?;
            *degree = degree.saturating_add(1);
            outgoing.entry(*predecessor).or_default().push(*digest);
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(digest, _)| *digest)
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::new();
    while let Some(node) = ready.pop_first() {
        ordered.push(node);
        let Some(successors) = outgoing.get(&node) else {
            continue;
        };
        for successor in successors {
            let degree = indegree.get_mut(successor)?;
            *degree = degree.saturating_sub(1);
            if *degree == 0 {
                ready.insert(*successor);
            }
        }
    }
    if ordered.len() == nodes.len() {
        Some(ordered)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::kahn_order;
    use crate::cbor_decode::{self, Value};
    use crate::diagnostics::ProblemEvidence;
    use crate::identity::{PROVENANCE_RECORD_DOMAIN, SNAPSHOT_DIGEST_DOMAIN, domain_separated};
    use crate::key::{ExternalInputKey, ExternalOutputKey, ModuleInstanceKey, NetworkKey};
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
    use crate::time::{NonZeroSpan, Time};
    use crate::{
        CompiledNetwork, ConflictPolicy, DecodeFailure, DecodePolicy, EdgeConfig,
        EdgeInitialization, FirstEmissionPolicy, InertialDelayConfig, InputDelta, InputSnapshot,
        LevelSetResetConfig, Machine, MachineSnapshot, ModuleBuilder, NetworkBuilder,
        PeriodicConfig, PersistenceContext, PulseDelayConfig, ReenablePhasePolicy, RestoreFailure,
        RuntimePolicy, SampleHoldConfig, TimeDomainId, ToggleConfig, Transaction,
        TransportDelayConfig, decode_snapshot,
    };

    const PREFIX: [u8; 8] = [0x4d, 0x53, 0x49, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

    fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(failure) => panic!("fixture step failed: {failure:?}"),
        }
    }

    fn policy() -> RuntimePolicy {
        policy_with(100)
    }

    fn policy_with(reactions: u64) -> RuntimePolicy {
        must(
            RuntimePolicy::builder()
                .max_internal_reactions(reactions)
                .max_evaluated_operations(10_000)
                .max_pending_events(100)
                .max_events_created_per_transaction(100)
                .max_required_provenance_growth(10_000)
                .build(),
        )
    }

    fn span(ticks: u64) -> NonZeroSpan<()> {
        must(NonZeroSpan::from_ticks(ticks))
    }

    fn compile(builder: NetworkBuilder<()>) -> CompiledNetwork<()> {
        let network = must(builder.finish().require_artifact());
        must(network.compile().require_artifact())
    }

    fn decode_policy() -> DecodePolicy {
        DecodePolicy::new(
            2_000_000, 64, 1_000_000, 1_000_000, 100_000, 10_000, 0, 0, 10_000, 10_000, 100_000,
            1_000_000, 10_000, 0, 1_000_000,
        )
    }

    fn limits(total_bytes: u64, nesting: u64, byte_string_bytes: u64) -> DecodePolicy {
        DecodePolicy::new(
            total_bytes,
            nesting,
            1_000_000,
            byte_string_bytes,
            100_000,
            10_000,
            0,
            0,
            10_000,
            10_000,
            100_000,
            1_000_000,
            10_000,
            0,
            1_000_000,
        )
    }

    fn toggle(domain: u128) -> (CompiledNetwork<()>, Machine<()>) {
        let meta = DiagnosticMeta::default();
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(domain));
        let input =
            must(builder.add_pulse_input(ExternalInputKey::<Pulse>::from_u128(6), meta.clone()));
        let toggle = must(builder.add_toggle(
            crate::key::NodeKey::from_u128(3),
            input,
            ToggleConfig::new(LogicLevel::Low),
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_level_output(ExternalOutputKey::<Level>::from_u128(8), toggle, meta));
        let compiled = compile(builder);
        let machine = compiled.spawn(policy());
        (compiled, machine)
    }

    fn families() -> (CompiledNetwork<()>, Machine<()>) {
        let meta = DiagnosticMeta::default();
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let level =
            must(builder.add_level_input(ExternalInputKey::<Level>::from_u128(10), meta.clone()));
        let pulse_a =
            must(builder.add_pulse_input(ExternalInputKey::<Pulse>::from_u128(11), meta.clone()));
        let pulse_b =
            must(builder.add_pulse_input(ExternalInputKey::<Pulse>::from_u128(12), meta.clone()));
        let high = must(builder.add_constant(
            crate::key::NodeKey::from_u128(13),
            LogicLevel::High,
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_rising_edge(
            crate::key::NodeKey::from_u128(20),
            level,
            EdgeConfig::new(EdgeInitialization::Baseline),
            meta.clone(),
        ));
        let toggle = must(builder.add_toggle(
            crate::key::NodeKey::from_u128(21),
            pulse_a,
            ToggleConfig::new(LogicLevel::Low),
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_level_set_reset_latch(
            crate::key::NodeKey::from_u128(22),
            high,
            high,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta.clone(),
        ));
        must(builder.add_level_set_reset_latch(
            crate::key::NodeKey::from_u128(23),
            high,
            high,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta.clone(),
        ));
        must(builder.add_sample_hold(
            crate::key::NodeKey::from_u128(24),
            level,
            pulse_a,
            SampleHoldConfig::new(LogicLevel::Low),
            meta.clone(),
        ));
        must(builder.add_pulse_delay(
            crate::key::NodeKey::from_u128(25),
            pulse_a,
            PulseDelayConfig::new(span(5)),
            meta.clone(),
        ));
        must(builder.add_pulse_delay(
            crate::key::NodeKey::from_u128(26),
            pulse_b,
            PulseDelayConfig::new(span(5)),
            meta.clone(),
        ));
        let transport = must(builder.add_transport_delay(
            crate::key::NodeKey::from_u128(27),
            level,
            TransportDelayConfig::new(span(4), LogicLevel::Low),
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_inertial_delay(
            crate::key::NodeKey::from_u128(28),
            level,
            InertialDelayConfig::new(span(6), LogicLevel::Low),
            meta.clone(),
        ));
        must(builder.add_periodic(
            crate::key::NodeKey::from_u128(29),
            level,
            PeriodicConfig::new(
                span(8),
                FirstEmissionPolicy::AfterFirstPeriod,
                ReenablePhasePolicy::PreservePhase,
            ),
            meta.clone(),
        ));
        must(builder.add_level_output(
            ExternalOutputKey::<Level>::from_u128(30),
            toggle,
            meta.clone(),
        ));
        must(builder.add_level_output(ExternalOutputKey::<Level>::from_u128(31), transport, meta));
        let compiled = compile(builder);
        let mut machine = compiled.spawn(policy());
        let snapshot = must(
            compiled
                .input_snapshot()
                .set(ExternalInputKey::<Level>::from_u128(10), LogicLevel::High)
                .and_then(|builder| {
                    builder.pulse(ExternalInputKey::<Pulse>::from_u128(11), PulseCount::ONE)
                })
                .and_then(|builder| {
                    builder.pulse(ExternalInputKey::<Pulse>::from_u128(12), PulseCount::ONE)
                })
                .and_then(crate::InputSnapshotBuilder::finish),
        );
        must(machine.apply(Transaction::initialize(
            Time::from_ticks(1),
            machine.revision(),
            snapshot,
        )));
        (compiled, machine)
    }

    fn standard_network() -> (CompiledNetwork<()>, Machine<()>) {
        let meta = DiagnosticMeta::default();
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(4), TimeDomainId::from_u128(2));
        let left =
            must(builder.add_level_input(ExternalInputKey::<Level>::from_u128(10), meta.clone()));
        let right =
            must(builder.add_level_input(ExternalInputKey::<Level>::from_u128(11), meta.clone()));
        let result = must(builder.exactly(1, [left, right]));
        must(builder.add_level_output(ExternalOutputKey::<Level>::from_u128(12), result, meta));
        let mut compiled = compile(builder);
        assert!(compiled.tamper_standard_expansion_for_test());
        let machine = compiled.spawn(policy());
        (compiled, machine)
    }

    fn encoded(machine: &Machine<()>) -> Vec<u8> {
        machine.snapshot().artifact_bytes().to_vec()
    }

    fn context_of(compiled: &CompiledNetwork<()>) -> PersistenceContext<()> {
        PersistenceContext::new(compiled.time_domain_id())
    }

    fn decode_with(
        compiled: &CompiledNetwork<()>,
        bytes: &[u8],
        policy: &DecodePolicy,
    ) -> Result<MachineSnapshot<()>, DecodeFailure<()>> {
        decode_snapshot(&context_of(compiled), bytes, policy)
    }

    fn restore_bytes(
        compiled: &CompiledNetwork<()>,
        bytes: &[u8],
        policy: RuntimePolicy,
    ) -> Result<Machine<()>, RestoreFailure<()>> {
        let snapshot = match decode_with(compiled, bytes, &decode_policy()) {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("decode failed: {}", show_decode(&failure)),
        };
        compiled.restore(snapshot, policy)
    }

    fn must_restore(
        compiled: &CompiledNetwork<()>,
        bytes: &[u8],
        policy: RuntimePolicy,
    ) -> Machine<()> {
        match restore_bytes(compiled, bytes, policy) {
            Ok(machine) => machine,
            Err(failure) => panic!("restore failed: {}", show_restore(&failure)),
        }
    }

    fn expect_restore(
        compiled: &CompiledNetwork<()>,
        bytes: &[u8],
        policy: RuntimePolicy,
        code: &str,
    ) -> RestoreFailure<()> {
        match restore_bytes(compiled, bytes, policy) {
            Ok(_) => panic!("expected {code}"),
            Err(failure) => {
                assert_eq!(failure.code().as_str(), code, "{}", show_restore(&failure));
                failure
            }
        }
    }

    fn show_decode(failure: &DecodeFailure<()>) -> String {
        format!(
            "{} {:?}",
            failure.code().as_str(),
            failure.problem().evidence()
        )
    }

    fn show_restore(failure: &RestoreFailure<()>) -> String {
        format!(
            "{} {:?}",
            failure.code().as_str(),
            failure.problem().evidence()
        )
    }

    fn assert_same(left: &Machine<()>, right: &Machine<()>) {
        assert_eq!(left.status(), right.status());
        assert_eq!(left.revision(), right.revision());
        assert_eq!(left.fingerprint(), right.fingerprint());
        assert_eq!(
            left.execution_state_digest(),
            right.execution_state_digest()
        );
        assert_eq!(
            left.observable_state_digest(),
            right.observable_state_digest()
        );
        assert_eq!(
            left.snapshot().artifact_bytes(),
            right.snapshot().artifact_bytes()
        );
    }

    fn contains(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|window| window == needle)
    }

    fn parse_artifact(bytes: &[u8]) -> Value {
        assert!(bytes.starts_with(&PREFIX), "artifact prefix");
        let body = &bytes[PREFIX.len()..];
        let (value, consumed) = must(cbor_decode::parse(
            body,
            cbor_decode::Limits {
                total_bytes: 2_000_000,
                nesting: 64,
                text_bytes: 1_000_000,
                byte_string_bytes: 1_000_000,
                collection_items: 100_000,
            },
        ));
        assert_eq!(consumed, body.len());
        value
    }

    fn prefix_encode(value: &Value) -> Vec<u8> {
        let mut bytes = PREFIX.to_vec();
        bytes.extend(cbor_decode::encode(value));
        bytes
    }

    fn envelope_mut(artifact: &mut Value) -> &mut Value {
        let Value::Array(items) = artifact else {
            panic!("wrapper");
        };
        &mut items[1]
    }

    fn payload_mut(artifact: &mut Value) -> &mut Value {
        record_field_mut(envelope_mut(artifact), "payload")
    }

    fn record_field<'a>(record: &'a Value, name: &str) -> &'a Value {
        let Value::Array(pairs) = record else {
            panic!("record");
        };
        for pair in pairs {
            if pair_name(pair) == name {
                let Value::Array(items) = pair else {
                    panic!("pair");
                };
                return &items[1];
            }
        }
        panic!("missing {name}");
    }

    fn record_field_mut<'a>(record: &'a mut Value, name: &str) -> &'a mut Value {
        let Value::Array(pairs) = record else {
            panic!("record");
        };
        for pair in pairs {
            let Value::Array(items) = pair else {
                panic!("pair");
            };
            let selected = matches!(items.first(), Some(Value::Text(found)) if found == name);
            if selected {
                return &mut items[1];
            }
        }
        panic!("missing {name}");
    }

    fn pair_name(pair: &Value) -> &str {
        let Value::Array(items) = pair else {
            panic!("pair");
        };
        let Value::Text(name) = &items[0] else {
            panic!("name");
        };
        name
    }

    fn remove_named(record: &mut Value, name: &str) {
        let Value::Array(pairs) = record else {
            panic!("record");
        };
        pairs.retain(|pair| pair_name(pair) != name);
    }

    fn insert_named(record: &mut Value, name: &str, value: Value) {
        let Value::Array(pairs) = record else {
            panic!("record");
        };
        let index = pairs
            .iter()
            .position(|pair| pair_name(pair) > name)
            .unwrap_or(pairs.len());
        pairs.insert(
            index,
            Value::Array(vec![Value::Text(name.to_owned()), value]),
        );
    }

    fn resign(artifact: &Value) -> Vec<u8> {
        let mut artifact = artifact.clone();
        remove_named(envelope_mut(&mut artifact), "integrity_digest");
        let bare = named_bytes(envelope_mut(&mut artifact));
        let digest = *blake3::hash(&domain_separated(SNAPSHOT_DIGEST_DOMAIN, 2, &bare)).as_bytes();
        insert_named(
            envelope_mut(&mut artifact),
            "integrity_digest",
            Value::Bytes(digest.to_vec()),
        );
        prefix_encode(&artifact)
    }

    fn named_bytes(record: &Value) -> Vec<u8> {
        let Value::Array(pairs) = record else {
            panic!("record");
        };
        let borrowed = pairs
            .iter()
            .map(|pair| {
                let Value::Array(items) = pair else {
                    panic!("pair");
                };
                let Value::Text(name) = &items[0] else {
                    panic!("name");
                };
                (name.as_str(), &items[1])
            })
            .collect::<Vec<_>>();
        cbor_decode::encode_named_pairs(&borrowed)
    }

    fn edited(bytes: &[u8], edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut artifact = parse_artifact(bytes);
        edit(&mut artifact);
        resign(&artifact)
    }

    fn raw(bytes: &[u8], edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut artifact = parse_artifact(bytes);
        edit(&mut artifact);
        prefix_encode(&artifact)
    }

    fn set_text(record: &mut Value, name: &str, text: &str) {
        *record_field_mut(record, name) = Value::Text(text.to_owned());
    }

    fn set_uint(record: &mut Value, name: &str, value: u64) {
        *record_field_mut(record, name) = Value::Uint(value);
    }

    fn flip_bytes(field: &mut Value) {
        let Value::Bytes(bytes) = field else {
            panic!("bytes");
        };
        bytes[0] ^= 0xff;
    }

    fn replace_first_text(value: &mut Value, from: &str, to: &str) -> bool {
        match value {
            Value::Text(text) if text == from => {
                *text = to.to_owned();
                true
            }
            Value::Array(items) => {
                for item in items {
                    if replace_first_text(item, from, to) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn set_first_named_uint(value: &mut Value, name: &str, new_value: u64) -> bool {
        let Value::Array(items) = value else {
            return false;
        };
        let selected =
            items.len() == 2 && matches!(items.first(), Some(Value::Text(found)) if found == name);
        if selected {
            items[1] = Value::Uint(new_value);
            return true;
        }
        for item in items {
            if set_first_named_uint(item, name, new_value) {
                return true;
            }
        }
        false
    }

    fn flip_first_key(value: &mut Value) -> bool {
        match value {
            Value::Bytes(bytes) if bytes.len() == 16 => {
                let last = bytes.len() - 1;
                bytes[last] ^= 0xff;
                true
            }
            Value::Array(items) => {
                for item in items {
                    if flip_first_key(item) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn payload_field_mut<'a>(artifact: &'a mut Value, name: &str) -> &'a mut Value {
        record_field_mut(payload_mut(artifact), name)
    }

    fn record_digest(record: &Value) -> [u8; 32] {
        let bytes = cbor_decode::encode(record);
        *blake3::hash(&domain_separated(PROVENANCE_RECORD_DOMAIN, 2, &bytes)).as_bytes()
    }

    fn provenance_items<'a>(artifact: &'a mut Value, name: &str) -> &'a mut Vec<Value> {
        let provenance = record_field_mut(payload_mut(artifact), "provenance");
        let field = record_field_mut(provenance, name);
        let Value::Array(items) = field else {
            panic!("{name}");
        };
        items
    }

    fn replace_digest(value: &mut Value, from: &[u8; 32], to: &[u8; 32]) {
        match value {
            Value::Bytes(bytes) if bytes.as_slice() == from.as_slice() => *bytes = to.to_vec(),
            Value::Array(items) => {
                for item in items {
                    replace_digest(item, from, to);
                }
            }
            _ => {}
        }
    }

    fn sort_provenance(artifact: &mut Value) {
        for name in [
            "episodes",
            "external_inputs",
            "output_baselines",
            "pending_events",
            "state",
        ] {
            let items = provenance_items(artifact, name);
            items.sort_by(|left, right| match (left, right) {
                (Value::Bytes(left), Value::Bytes(right)) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            });
        }
        let records = provenance_items(artifact, "records");
        records.sort_by_key(record_digest);
    }

    fn contains_text(value: &Value, needle: &str) -> bool {
        match value {
            Value::Text(text) => text == needle,
            Value::Array(items) => items.iter().any(|item| contains_text(item, needle)),
            _ => false,
        }
    }

    fn rename_one_role(artifact: &mut Value) {
        let records = provenance_items(artifact, "records").clone();
        let index = records
            .iter()
            .position(|record| contains_text(record, "supporter"))
            .expect("supporter role");
        let mut edited = records[index].clone();
        let old = record_digest(&edited);
        assert!(replace_first_text(&mut edited, "supporter", "ancestors"));
        let new = record_digest(&edited);
        provenance_items(artifact, "records")[index] = edited;
        replace_digest(artifact, &old, &new);
        sort_provenance(artifact);
    }

    fn first_named_bytes(value: &Value, name: &str) -> Option<[u8; 32]> {
        let Value::Array(items) = value else {
            return None;
        };
        if items.len() == 2 && matches!(&items[0], Value::Text(found) if found == name) {
            if let Value::Bytes(bytes) = &items[1] {
                if bytes.len() == 32 {
                    let mut digest = [0; 32];
                    digest.copy_from_slice(bytes);
                    return Some(digest);
                }
            }
            return None;
        }
        for item in items {
            if let Some(found) = first_named_bytes(item, name) {
                return Some(found);
            }
        }
        None
    }

    fn drop_cited_record(artifact: &mut Value) {
        let records = provenance_items(artifact, "records").clone();
        let digests = records.iter().map(record_digest).collect::<Vec<_>>();
        let mut victim = None;
        for record in &records {
            if let Some(cited) = first_named_bytes(record, "predecessor") {
                if digests.contains(&cited) {
                    victim = Some(cited);
                    break;
                }
            }
        }
        let victim = victim.expect("cited predecessor");
        provenance_items(artifact, "records").retain(|record| record_digest(record) != victim);
    }

    fn duplicate_first_record(artifact: &mut Value) {
        let records = provenance_items(artifact, "records");
        let copy = records[0].clone();
        records.insert(1, copy);
    }

    fn graft_episode(target: &mut Value, source: &Value) {
        let episode = {
            let payload = record_field(envelope(source), "payload");
            let episodes = record_field(payload, "active_diagnostic_episodes");
            let Value::Array(items) = episodes else {
                panic!("episodes");
            };
            items[0].clone()
        };
        let episodes = payload_field_mut(target, "active_diagnostic_episodes");
        let Value::Array(items) = episodes else {
            panic!("episodes");
        };
        items.push(episode);
    }

    fn envelope(artifact: &Value) -> &Value {
        let Value::Array(items) = artifact else {
            panic!("wrapper");
        };
        &items[1]
    }

    fn version_evidence(failure: &DecodeFailure<()>) -> (String, String, bool) {
        match failure.problem().evidence() {
            ProblemEvidence::PersistenceUnsupportedVersion { evidence, .. } => (
                evidence.component.clone(),
                evidence.required.clone(),
                evidence.upgrader_exists,
            ),
            other => panic!("version evidence, got {other:?}"),
        }
    }

    fn budget_name(failure: &DecodeFailure<()>) -> &'static str {
        match failure.problem().evidence() {
            ProblemEvidence::PersistenceDecodeLimitExceeded { evidence, .. } => evidence.budget,
            other => panic!("budget evidence, got {other:?}"),
        }
    }

    fn init_pulse(compiled: &CompiledNetwork<()>) -> InputSnapshot<()> {
        let builder = must(
            compiled
                .input_snapshot()
                .pulse(ExternalInputKey::<Pulse>::from_u128(6), PulseCount::ONE),
        );
        must(builder.finish())
    }

    fn empty_delta(compiled: &CompiledNetwork<()>) -> InputDelta<()> {
        must(compiled.input_delta().finish())
    }

    #[test]
    fn uninitialized_round_trip_then_initializes_like_the_original() {
        let (compiled, mut original) = toggle(2);
        let bytes = encoded(&original);
        let restored = must_restore(&compiled, &bytes, policy());
        assert_same(&original, &restored);
        let mut restored = restored;
        must(original.apply(Transaction::initialize(
            Time::from_ticks(1),
            original.revision(),
            init_pulse(&compiled),
        )));
        must(restored.apply(Transaction::initialize(
            Time::from_ticks(1),
            restored.revision(),
            init_pulse(&compiled),
        )));
        assert_same(&original, &restored);
    }

    #[test]
    fn families_round_trip_keeps_pulse_groups_without_a_checkpoint_and_advances() {
        let (compiled, mut original) = families();
        let bytes = encoded(&original);
        assert!(contains(&bytes, b"pulse_delay_group"));
        assert!(contains(&bytes, b"complete_from_initialization"));
        assert!(!contains(&bytes, b"authoritative_checkpoint"));
        let mut restored = must_restore(&compiled, &bytes, policy());
        assert_same(&original, &restored);
        let at = Time::from_ticks(10);
        must(original.apply(Transaction::advance(
            at,
            original.revision(),
            empty_delta(&compiled),
        )));
        must(restored.apply(Transaction::advance(
            at,
            restored.revision(),
            empty_delta(&compiled),
        )));
        assert_same(&original, &restored);
    }

    #[test]
    fn zero_port_connection_and_replay_limits_still_decode() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        assert!(decode_with(&compiled, &bytes, &decode_policy()).is_ok());
    }

    #[test]
    fn framing_kind_and_integrity_failures_do_not_change_caller_bytes() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let before = bytes.clone();
        let cases = [
            (Vec::new(), "persistence.truncated_artifact"),
            (PREFIX[..4].to_vec(), "persistence.truncated_artifact"),
            (b"not-msig".to_vec(), "persistence.invalid_prefix"),
            (PREFIX.to_vec(), "persistence.truncated_artifact"),
        ];
        for (case, code) in cases {
            let owned = case.clone();
            let failure = decode_with(&compiled, &case, &decode_policy()).expect_err(code);
            assert_eq!(failure.code().as_str(), code);
            assert_eq!(case, owned);
        }
        let mut trailed = bytes.clone();
        trailed.push(0);
        let failure = decode_with(&compiled, &trailed, &decode_policy()).expect_err("trailing");
        assert_eq!(failure.code().as_str(), "persistence.trailing_bytes");
        let noncanonical = {
            let mut body = PREFIX.to_vec();
            body.extend([0x18, 0x01]);
            body
        };
        let failure = decode_with(&compiled, &noncanonical, &decode_policy()).expect_err("form");
        assert_eq!(failure.code().as_str(), "persistence.noncanonical_encoding");
        match failure.problem().evidence() {
            ProblemEvidence::PersistenceNoncanonicalEncoding { evidence, .. } => {
                assert_eq!(evidence.violation, "non_shortest_integer");
            }
            other => panic!("{other:?}"),
        }
        let huge = {
            let mut body = PREFIX.to_vec();
            body.push(0x5b);
            body.extend(4_294_967_296u64.to_be_bytes());
            body
        };
        let failure = decode_with(&compiled, &huge, &limits(1_000, 64, 100)).expect_err("limit");
        assert_eq!(failure.code().as_str(), "persistence.decode_limit_exceeded");
        assert_eq!(budget_name(&failure), "byte_string_bytes");
        let modest = {
            let mut body = PREFIX.to_vec();
            body.extend([0x45, 0x00]);
            body
        };
        let failure = decode_with(&compiled, &modest, &decode_policy()).expect_err("short");
        assert_eq!(failure.code().as_str(), "persistence.truncated_artifact");
        let nested =
            decode_with(&compiled, &bytes, &limits(2_000_000, 1, 1_000_000)).expect_err("nest");
        assert_eq!(nested.code().as_str(), "persistence.decode_limit_exceeded");
        assert_eq!(budget_name(&nested), "nesting");
        let migration = raw(&bytes, |artifact| {
            set_text(envelope_mut(artifact), "artifact_kind", "migration_report");
        });
        let failure = decode_with(&compiled, &migration, &decode_policy()).expect_err("kind");
        assert_eq!(failure.code().as_str(), "persistence.malformed_envelope");
        let unknown = raw(&bytes, |artifact| {
            set_text(envelope_mut(artifact), "artifact_kind", "not_a_snapshot!!");
        });
        let failure = decode_with(&compiled, &unknown, &decode_policy()).expect_err("unknown");
        assert_eq!(failure.code().as_str(), "persistence.unknown_artifact_kind");
        let missing = raw(&bytes, |artifact| {
            remove_named(envelope_mut(artifact), "integrity_digest");
        });
        let failure = decode_with(&compiled, &missing, &decode_policy()).expect_err("integrity");
        assert_eq!(failure.code().as_str(), "persistence.malformed_envelope");
        let flipped = raw(&bytes, |artifact| {
            flip_bytes(record_field_mut(envelope_mut(artifact), "integrity_digest"));
        });
        let failure = decode_with(&compiled, &flipped, &decode_policy()).expect_err("digest");
        assert_eq!(
            failure.code().as_str(),
            "persistence.integrity_digest_mismatch"
        );
        let wrapped = MachineSnapshot::from_decoded(
            machine.snapshot().status(),
            machine.snapshot().revision(),
            machine.snapshot().fingerprint(),
            machine.snapshot().execution_state_digest(),
            machine.snapshot().observable_state_digest(),
            machine.snapshot().snapshot_digest(),
            machine.snapshot().runtime_policy_id(),
            compiled.time_domain_id(),
            migration,
        );
        let restored = compiled.restore(wrapped, policy());
        assert!(matches!(
            restored,
            Err(RestoreFailure::Decode(DecodeFailure::MalformedEnvelope(_)))
        ));
        assert_eq!(bytes, before);
    }

    #[test]
    fn resigned_version_three_is_unsupported_and_payload_disagreement_matches() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let versioned = edited(&bytes, |artifact| {
            set_uint(envelope_mut(artifact), "artifact_schema_version", 3);
        });
        let failure = decode_with(&compiled, &versioned, &decode_policy()).expect_err("version");
        assert_eq!(failure.code().as_str(), "persistence.unsupported_version");
        assert!(matches!(failure, DecodeFailure::UnsupportedVersion(_)));
        let (component, required, upgrader) = version_evidence(&failure);
        assert_eq!(component, "artifact_schema_version");
        assert_eq!(required, "2");
        assert!(!upgrader);
        let disagreed = edited(&bytes, |artifact| {
            let versions = record_field_mut(payload_mut(artifact), "semantic_versions");
            set_uint(versions, "core_semantics_version", 3);
        });
        let failure = decode_with(&compiled, &disagreed, &decode_policy()).expect_err("payload");
        assert_eq!(failure.code().as_str(), "persistence.unsupported_version");
        let (component, required, upgrader) = version_evidence(&failure);
        assert_eq!(component, "core_semantics_version");
        assert_eq!(required, "2");
        assert!(!upgrader);
    }

    #[test]
    fn time_domain_failures_use_both_decode_and_restore_leaves() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let other = PersistenceContext::<()>::new(TimeDomainId::from_u128(9));
        let failure = decode_snapshot(&other, &bytes, &decode_policy()).expect_err("context");
        assert!(matches!(failure, DecodeFailure::WrongTimeDomain(_)));
        assert_eq!(failure.code().as_str(), "persistence.wrong_time_domain");
        let (elsewhere, _) = toggle(9);
        let snapshot = must(decode_with(&compiled, &bytes, &decode_policy()));
        let restored = elsewhere.restore(snapshot, policy());
        assert!(matches!(restored, Err(RestoreFailure::WrongTimeDomain(_))));
        let mismatched = edited(&bytes, |artifact| {
            flip_bytes(record_field_mut(payload_mut(artifact), "time_domain_id"));
        });
        let failure = decode_with(&compiled, &mismatched, &decode_policy()).expect_err("payload");
        assert!(matches!(failure, DecodeFailure::WrongTimeDomain(_)));
    }

    #[test]
    fn identity_policy_and_expansion_mismatches_publish_nothing() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let fingerprint_before = compiled.fingerprint();
        let wrong_key = edited(&bytes, |artifact| {
            flip_bytes(payload_field_mut(artifact, "network_key"));
        });
        expect_restore(
            &compiled,
            &wrong_key,
            policy(),
            "persistence.network_identity_mismatch",
        );
        let wrong_fingerprint = edited(&bytes, |artifact| {
            flip_bytes(payload_field_mut(artifact, "network_fingerprint"));
        });
        expect_restore(
            &compiled,
            &wrong_fingerprint,
            policy(),
            "persistence.fingerprint_mismatch",
        );
        let snapshot = must(decode_with(&compiled, &bytes, &decode_policy()));
        let kept = snapshot.clone();
        let restored = compiled.restore(snapshot, policy_with(101));
        assert!(matches!(
            restored,
            Err(RestoreFailure::RuntimePolicyMismatch(_))
        ));
        assert_eq!(kept.artifact_bytes(), bytes.as_slice());
        assert_eq!(compiled.fingerprint(), fingerprint_before);
        let (tampered, standard) = standard_network();
        expect_restore(
            &tampered,
            &encoded(&standard),
            policy(),
            "standard_module.expansion_mismatch",
        );
    }

    #[test]
    fn state_lifecycle_pending_and_episode_faults_reject_the_candidate() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let schema = edited(&bytes, |artifact| {
            let table = payload_field_mut(artifact, "node_state_table");
            assert!(replace_first_text(
                table,
                "stored_level",
                "edge_observation"
            ));
        });
        expect_restore(
            &compiled,
            &schema,
            policy(),
            "persistence.state_schema_mismatch",
        );
        let unknown = edited(&bytes, |artifact| {
            let table = payload_field_mut(artifact, "node_state_table");
            assert!(flip_first_key(table));
        });
        expect_restore(&compiled, &unknown, policy(), "persistence.unknown_subject");
        let (uninitialized, idle) = toggle(2);
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let family = parse_artifact(&bytes);
        let lifecycle = edited(&encoded(&idle), |artifact| graft_episode(artifact, &family));
        expect_restore(
            &uninitialized,
            &lifecycle,
            policy(),
            "persistence.lifecycle_shape_invalid",
        );
        let revision = edited(&bytes, |artifact| {
            let lifecycle = payload_field_mut(artifact, "lifecycle");
            assert!(set_first_named_uint(lifecycle, "origin_revision", 5));
        });
        expect_restore(
            &compiled,
            &revision,
            policy(),
            "persistence.topology_revision_mismatch",
        );
        let deadline = edited(&bytes, |artifact| {
            let lifecycle = payload_field_mut(artifact, "lifecycle");
            assert!(set_first_named_uint(lifecycle, "deadline", 1));
        });
        expect_restore(
            &compiled,
            &deadline,
            policy(),
            "persistence.pending_event_invalid",
        );
        let link = edited(&bytes, |artifact| {
            let temporal = payload_field_mut(artifact, "temporal_state_table");
            assert!(set_first_named_uint(temporal, "event", u64::MAX));
        });
        expect_restore(
            &compiled,
            &link,
            policy(),
            "persistence.event_identity_state_invalid",
        );
        let code = edited(&bytes, |artifact| {
            let episodes = payload_field_mut(artifact, "active_diagnostic_episodes");
            assert!(replace_first_text(
                episodes,
                "runtime.level_latch_conflict_retained",
                "runtime.unknown_episode_code",
            ));
        });
        expect_restore(
            &compiled,
            &code,
            policy(),
            "persistence.diagnostic_schema_invalid",
        );
        let episode = edited(&bytes, |artifact| {
            let episodes = payload_field_mut(artifact, "active_diagnostic_episodes");
            assert!(set_first_named_uint(episodes, "discriminator", 1));
        });
        expect_restore(
            &compiled,
            &episode,
            policy(),
            "persistence.diagnostic_episode_invalid",
        );
    }

    #[test]
    fn provenance_role_predecessor_duplicate_and_false_checkpoint_fail_closed() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let role = edited(&bytes, rename_one_role);
        expect_restore(
            &compiled,
            &role,
            policy(),
            "persistence.provenance_invalid_role",
        );
        let missing = edited(&bytes, drop_cited_record);
        expect_restore(
            &compiled,
            &missing,
            policy(),
            "persistence.provenance_missing_predecessor",
        );
        let duplicate = edited(&bytes, duplicate_first_record);
        expect_restore(
            &compiled,
            &duplicate,
            policy(),
            "persistence.provenance_conflicting_record",
        );
        let checkpoint = edited(&bytes, |artifact| {
            let lifecycle = payload_field_mut(artifact, "lifecycle");
            assert!(replace_first_text(
                lifecycle,
                "complete_from_initialization",
                "checkpoint",
            ));
        });
        let failure = expect_restore(
            &compiled,
            &checkpoint,
            policy(),
            "persistence.provenance_false_checkpoint",
        );
        match failure.problem().evidence() {
            ProblemEvidence::PersistenceProvenanceFalseCheckpoint { evidence, .. } => {
                assert_eq!(evidence.checkpoint, "checkpoint");
                assert!(evidence.subject.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ordered_pending_and_phase_stamps_cannot_exceed_the_committed_occurrence() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let future_origin = edited(&bytes, |artifact| {
            let pending = record_field_mut(ready_body_mut(artifact), "pending_events");
            assert!(set_first_named_uint(pending, "stimulus_order", 1));
        });
        expect_restore(
            &compiled,
            &future_origin,
            policy(),
            "persistence.pending_event_invalid",
        );
        for field in ["phase_order", "settled_boundary"] {
            let future_phase = edited(&bytes, |artifact| {
                let state = payload_field_mut(artifact, "node_state_table");
                assert!(set_first_named_uint(state, field, 2));
            });
            expect_restore(
                &compiled,
                &future_phase,
                policy(),
                "persistence.lifecycle_shape_invalid",
            );
        }
        let missing_order = edited(&bytes, |artifact| {
            remove_named(ready_body_mut(artifact), "reaction_order");
        });
        assert!(decode_with(&compiled, &missing_order, &decode_policy()).is_err());
    }

    fn disabled_preserved_periodic(at: u64) -> (CompiledNetwork<()>, Machine<()>) {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(77), TimeDomainId::from_u128(2));
        let input = ExternalInputKey::<Level>::from_u128(10);
        let enable = must(builder.add_level_input(input, DiagnosticMeta::default()));
        let pulse = must(builder.add_periodic(
            crate::key::NodeKey::from_u128(29),
            enable,
            PeriodicConfig::new(
                span(5),
                FirstEmissionPolicy::Immediate,
                ReenablePhasePolicy::PreservePhase,
            ),
            DiagnosticMeta::default(),
        ))
        .into_outputs();
        must(builder.add_pulse_output(
            ExternalOutputKey::<Pulse>::from_u128(40),
            pulse,
            DiagnosticMeta::default(),
        ));
        let compiled = compile(builder);
        let mut machine = compiled.spawn(policy());
        let initial = must(must(compiled.input_snapshot().set(input, LogicLevel::High)).finish());
        must(machine.apply(Transaction::initialize(
            Time::from_ticks(0),
            machine.revision(),
            initial,
        )));
        let disabled = must(must(compiled.input_delta().set(input, LogicLevel::Low)).finish());
        must(machine.apply(Transaction::advance(
            Time::from_ticks(1),
            machine.revision(),
            disabled,
        )));
        must(machine.apply(Transaction::advance(
            Time::from_ticks(at),
            machine.revision(),
            empty_delta(&compiled),
        )));
        (compiled, machine)
    }

    fn reject_damaged_periodic_watermark(settled: Option<Time<()>>) {
        let (compiled, mut machine) = disabled_preserved_periodic(5);
        let phase = machine
            .store
            .periodic_anchors
            .get_mut(&crate::key::NodeKey::from_u128(29))
            .unwrap();
        assert_eq!(phase.settled, Some(Time::from_ticks(5)));
        phase.settled = settled;
        // Re-encode the mutated semantic store so every digest is correct for
        // the malformed continuation; rejection must come from validation.
        let bytes = encoded(&machine);
        let snapshot = must(decode_with(&compiled, &bytes, &decode_policy()));
        assert_eq!(
            snapshot.execution_state_digest(),
            machine.execution_state_digest()
        );
        assert_eq!(
            snapshot.observable_state_digest(),
            machine.observable_state_digest()
        );
        match restore_bytes(&compiled, &bytes, policy()) {
            Ok(mut restored) => {
                let enabled = must(
                    must(
                        compiled
                            .input_delta()
                            .set(ExternalInputKey::<Level>::from_u128(10), LogicLevel::High),
                    )
                    .finish(),
                );
                let replayed = must(restored.apply(Transaction::advance(
                    Time::from_ticks(5),
                    restored.revision(),
                    enabled,
                )));
                assert!(
                    matches!(replayed.output_events(), [crate::OutputEvent::Pulsed { count, stamp, .. }] if *count == PulseCount::ONE && stamp.time() == Time::from_ticks(5))
                );
                panic!("restore accepted a damaged watermark and replayed the suppressed boundary");
            }
            Err(failure) => assert_eq!(
                failure.code().as_str(),
                "persistence.lifecycle_shape_invalid"
            ),
        }
    }

    #[test]
    fn periodic_watermark_missing_at_settled_boundary_is_rejected() {
        reject_damaged_periodic_watermark(None);
    }

    #[test]
    fn periodic_watermark_regressed_at_settled_boundary_is_rejected() {
        reject_damaged_periodic_watermark(Some(Time::from_ticks(0)));
    }

    #[test]
    fn periodic_watermark_preserves_valid_boundaries_and_unvisited_disabled_gaps() {
        for (at, expected_settled, next) in [(5, 5, 10), (6, 0, 10), (11, 0, 15)] {
            let (compiled, mut original) = disabled_preserved_periodic(at);
            let node = crate::key::NodeKey::from_u128(29);
            assert_eq!(
                original.inspect_periodic(node).unwrap().settled_boundary(),
                Some(Time::from_ticks(expected_settled))
            );
            let mut restored = must_restore(&compiled, &encoded(&original), policy());
            assert_same(&original, &restored);
            let enabled = must(
                must(
                    compiled
                        .input_delta()
                        .set(ExternalInputKey::<Level>::from_u128(10), LogicLevel::High),
                )
                .finish(),
            );
            let tx = Transaction::advance(Time::from_ticks(at), original.revision(), enabled);
            let result = must(restored.apply(tx.clone()));
            must(original.apply(tx));
            assert!(result.output_events().is_empty());
            assert_same(&original, &restored);
            assert_eq!(
                restored.inspect_periodic(node).unwrap().next_deadline(),
                Some(Time::from_ticks(next))
            );
            let due = must(restored.apply(Transaction::advance(
                Time::from_ticks(next),
                restored.revision(),
                empty_delta(&compiled),
            )));
            assert!(
                matches!(due.output_events(), [crate::OutputEvent::Pulsed { count, .. }] if *count == PulseCount::ONE)
            );
        }
    }

    #[test]
    fn digest_mismatches_and_optional_history_leave_the_machine_unchanged() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let execution = edited(&bytes, |artifact| {
            flip_bytes(payload_field_mut(artifact, "execution_state_digest"));
        });
        expect_restore(
            &compiled,
            &execution,
            policy(),
            "persistence.execution_digest_mismatch",
        );
        let observable = edited(&bytes, |artifact| {
            flip_bytes(payload_field_mut(artifact, "observable_state_digest"));
        });
        expect_restore(
            &compiled,
            &observable,
            policy(),
            "persistence.observable_digest_mismatch",
        );
        let historic = edited(&bytes, |artifact| {
            insert_named(
                payload_mut(artifact),
                "optional_history",
                Value::Text("kept".to_owned()),
            );
        });
        let restored = must_restore(&compiled, &historic, policy());
        assert_eq!(
            restored.execution_state_digest(),
            machine.execution_state_digest()
        );
        assert_eq!(
            restored.observable_state_digest(),
            machine.observable_state_digest()
        );
        assert_eq!(restored.status(), machine.status());
        assert!(!contains(restored.snapshot().artifact_bytes(), b"kept"));
        let labeled = edited(&bytes, |artifact| {
            insert_named(
                payload_mut(artifact),
                "persistence_metadata",
                Value::Array(vec![Value::Array(vec![
                    Value::Text("label".to_owned()),
                    Value::Text("bench".to_owned()),
                ])]),
            );
        });
        let restored = must_restore(&compiled, &labeled, policy());
        assert_eq!(
            restored.execution_state_digest(),
            machine.execution_state_digest()
        );
        assert_eq!(
            restored.observable_state_digest(),
            machine.observable_state_digest()
        );
        assert!(!contains(restored.snapshot().artifact_bytes(), b"bench"));
    }

    #[test]
    fn kahn_order_rejects_cycles_and_orders_a_chain() {
        let left = [1_u8; 32];
        let right = [2_u8; 32];
        let mut cycle = std::collections::BTreeMap::new();
        cycle.insert(left, vec![right]);
        cycle.insert(right, vec![left]);
        assert!(kahn_order(&cycle).is_none());
        let mut self_loop = std::collections::BTreeMap::new();
        self_loop.insert(left, vec![left]);
        assert!(kahn_order(&self_loop).is_none());
        let mut outside = std::collections::BTreeMap::new();
        outside.insert(left, vec![[3_u8; 32]]);
        assert!(kahn_order(&outside).is_none());
        let mut chain = std::collections::BTreeMap::new();
        chain.insert(left, Vec::new());
        chain.insert(right, vec![left]);
        assert_eq!(kahn_order(&chain), Some(vec![left, right]));
    }

    fn payload(artifact: &Value) -> &Value {
        record_field(envelope(artifact), "payload")
    }

    fn uint_field(record: &Value, name: &str) -> u64 {
        match record_field(record, name) {
            Value::Uint(value) => *value,
            other => panic!("{name}: {other:?}"),
        }
    }

    fn ready_body_mut(artifact: &mut Value) -> &mut Value {
        let lifecycle = payload_field_mut(artifact, "lifecycle");
        let Value::Array(items) = lifecycle else {
            panic!("lifecycle");
        };
        &mut items[1]
    }

    fn null_singular_with_second_inertial_event(artifact: &mut Value) {
        let next = uint_field(payload(artifact), "next_pending_event_serial");
        set_uint(payload_mut(artifact), "next_pending_event_serial", next + 1);
        {
            let ready = ready_body_mut(artifact);
            let Value::Array(events) = record_field_mut(ready, "pending_events") else {
                panic!("pending");
            };
            let index = events
                .iter()
                .position(|event| contains_text(event, "inertial_maturation"))
                .expect("inertial event");
            let mut extra = events[index].clone();
            set_uint(&mut extra, "key", next);
            events.insert(index + 1, extra);
        }
        let table = payload_field_mut(artifact, "temporal_state_table");
        assert!(null_schema_value(table, "pending_inertial_candidate"));
    }

    fn null_schema_value(table: &mut Value, schema: &str) -> bool {
        let Value::Array(entries) = table else {
            return false;
        };
        for entry in entries {
            if contains_text(entry, schema) {
                *record_field_mut(entry, "value") = Value::Null;
                return true;
            }
        }
        false
    }

    fn flip_settled_level(artifact: &mut Value) {
        let ready = ready_body_mut(artifact);
        let settled = record_field_mut(ready, "settled_levels");
        assert!(flip_first_level(settled), "settled level");
    }

    fn flip_first_level(value: &mut Value) -> bool {
        match value {
            Value::Text(text) if text == "low" || text == "high" => {
                *text = if text == "low" {
                    "high".to_owned()
                } else {
                    "low".to_owned()
                };
                true
            }
            Value::Array(items) => items.iter_mut().any(flip_first_level),
            _ => false,
        }
    }

    fn set_first_supporter_payload(artifact: &mut Value) {
        let records = provenance_items(artifact, "records").clone();
        let index = records
            .iter()
            .position(|record| {
                contains_text(record, "supporter") && record_has_named(record, "predecessors")
            })
            .expect("supporter");
        let mut edited = records[index].clone();
        let old = record_digest(&edited);
        {
            let predecessors = record_field_mut(&mut edited, "predecessors");
            let Value::Array(items) = predecessors else {
                panic!("predecessors");
            };
            let slot = items
                .iter()
                .position(|predecessor| record_text(predecessor, "role") == Some("supporter"))
                .expect("supporter role");
            *record_field_mut(&mut items[slot], "payload") = Value::Uint(1);
        }
        let new = record_digest(&edited);
        provenance_items(artifact, "records")[index] = edited;
        replace_digest(artifact, &old, &new);
        sort_provenance(artifact);
    }

    fn record_has_named(record: &Value, name: &str) -> bool {
        let Value::Array(pairs) = record else {
            return false;
        };
        pairs.iter().any(|pair| pair_name(pair) == name)
    }

    fn record_text<'a>(record: &'a Value, name: &str) -> Option<&'a str> {
        match record_field(record, name) {
            Value::Text(text) => Some(text.as_str()),
            _ => None,
        }
    }

    fn omit_ready_edge_cause(
        artifact: &mut Value,
        dropped: &std::collections::BTreeSet<[u8; 32]>,
        edge: [u8; 32],
        execution: [u8; 32],
        observable: [u8; 32],
    ) {
        for name in [
            "episodes",
            "external_inputs",
            "output_baselines",
            "pending_events",
            "state",
        ] {
            let state_list = name == "state";
            let items = provenance_items(artifact, name);
            items.retain(|item| !listed_digest_removed(item, dropped, state_list, edge));
        }
        let records = provenance_items(artifact, "records");
        records.retain(|record| !dropped.contains(&record_digest(record)));
        *payload_field_mut(artifact, "execution_state_digest") = Value::Bytes(execution.to_vec());
        *payload_field_mut(artifact, "observable_state_digest") = Value::Bytes(observable.to_vec());
    }

    fn listed_digest_removed(
        item: &Value,
        dropped: &std::collections::BTreeSet<[u8; 32]>,
        state_list: bool,
        edge: [u8; 32],
    ) -> bool {
        let Value::Bytes(bytes) = item else {
            return false;
        };
        if bytes.len() != 32 {
            return false;
        }
        let mut digest = [0; 32];
        digest.copy_from_slice(bytes);
        dropped.contains(&digest) || (state_list && digest == edge)
    }

    fn key_cbor(value: u128) -> Vec<u8> {
        let mut bytes = vec![0x50];
        bytes.extend(value.to_be_bytes());
        bytes
    }

    #[test]
    fn total_byte_budget_counts_the_prefix() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let len = bytes.len() as u64;
        let tight =
            decode_with(&compiled, &bytes, &limits(len - 1, 64, 1_000_000)).expect_err("budget");
        assert_eq!(tight.code().as_str(), "persistence.decode_limit_exceeded");
        assert_eq!(budget_name(&tight), "total_bytes");
        assert!(decode_with(&compiled, &bytes, &limits(len, 64, 1_000_000)).is_ok());
    }

    #[test]
    fn missing_envelope_version_is_malformed() {
        let (compiled, machine) = toggle(2);
        let bytes = encoded(&machine);
        let missing = edited(&bytes, |artifact| {
            remove_named(envelope_mut(artifact), "artifact_schema_version");
        });
        let failure = decode_with(&compiled, &missing, &decode_policy()).expect_err("version");
        assert_eq!(failure.code().as_str(), "persistence.malformed_envelope");
    }

    #[test]
    fn null_singular_reference_rejects_two_inertial_events() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let doubled = edited(&bytes, null_singular_with_second_inertial_event);
        expect_restore(
            &compiled,
            &doubled,
            policy(),
            "persistence.event_identity_state_invalid",
        );
    }

    #[test]
    fn tampered_settled_level_is_inconsistent() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let tampered = edited(&bytes, flip_settled_level);
        expect_restore(
            &compiled,
            &tampered,
            policy(),
            "persistence.settled_state_inconsistent",
        );
    }

    #[test]
    fn non_null_supporter_payload_is_malformed() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let corrupted = edited(&bytes, set_first_supporter_payload);
        let failure = decode_with(&compiled, &corrupted, &decode_policy()).expect_err("supporter");
        assert_eq!(failure.code().as_str(), "persistence.malformed_envelope");
    }

    #[test]
    fn ready_edge_without_its_cause_is_an_incomplete_root() {
        let (compiled, machine) = families();
        let bytes = encoded(&machine);
        let mut damaged = must_restore(&compiled, &bytes, policy());
        let edge = crate::key::NodeKey::from_u128(20);
        let cause = damaged
            .store
            .edge_observation_causes
            .get(&edge)
            .copied()
            .expect("rising edge cause");
        let ordinal = damaged
            .store
            .provenance
            .as_ref()
            .expect("provenance")
            .resolve_ordinal(cause);
        let before = crate::state_digest::cause_digest_index(&damaged);
        let edge_digest = before.machine_digest(ordinal);
        damaged.store.edge_observation_causes.remove(&edge);
        let after = crate::state_digest::cause_digest_index(&damaged);
        let dropped = before
            .records()
            .keys()
            .copied()
            .filter(|digest| !after.records().contains_key(digest))
            .collect::<std::collections::BTreeSet<_>>();
        let execution = damaged.execution_state_digest().as_bytes();
        let observable = damaged.observable_state_digest().as_bytes();
        let edited_bytes = edited(&bytes, |artifact| {
            omit_ready_edge_cause(artifact, &dropped, edge_digest, execution, observable);
        });
        expect_restore(
            &compiled,
            &edited_bytes,
            policy(),
            "persistence.provenance_incomplete_root_closure",
        );
    }

    #[test]
    fn module_qualified_owner_restores_and_initializes() {
        let mut inner = ModuleBuilder::new();
        let (inner_pulse, pulse) = inner.pulse_input("toggle");
        let level = must(inner.toggle(pulse, ToggleConfig::new(LogicLevel::Low)));
        let inner_level = must(inner.level_output("level", level));
        let inner = match inner.finish().require_artifact() {
            Ok(module) => module,
            Err(failure) => panic!("inner module must validate: {failure:?}"),
        };
        let nested = ModuleInstanceKey::from_u128(0x50);
        let mut outer = ModuleBuilder::new();
        let (outer_pulse, pulse) = outer.pulse_input("toggle");
        let added = must(
            must(outer.instantiate(&inner, nested, DiagnosticMeta::default()))
                .bind_pulse(inner_pulse, pulse),
        )
        .finish()
        .unwrap_or_else(|failure| panic!("nested instance must finish: {failure:?}"));
        let outer_level = must(outer.level_output("level", must(added.level_output(inner_level))));
        let outer = match outer.finish().require_artifact() {
            Ok(module) => module,
            Err(failure) => panic!("outer module must validate: {failure:?}"),
        };
        let instance = ModuleInstanceKey::from_u128(0x61);
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let signal = must(builder.add_pulse_input(
            ExternalInputKey::<Pulse>::from_u128(0x71),
            DiagnosticMeta::default(),
        ));
        let added = must(
            must(builder.instantiate(&outer, instance, DiagnosticMeta::default()))
                .bind_pulse(outer_pulse, signal),
        )
        .finish()
        .unwrap_or_else(|failure| panic!("outer instance must finish: {failure:?}"));
        must(builder.add_level_output(
            ExternalOutputKey::<Level>::from_u128(0x81),
            must(added.level_output(outer_level)),
            DiagnosticMeta::default(),
        ));
        let compiled = compile(builder);
        let mut original = compiled.spawn(policy());
        let bytes = encoded(&original);
        let mut path = vec![0x82];
        path.extend(key_cbor(0x61));
        path.extend(key_cbor(0x50));
        assert!(contains(&bytes, &path), "qualified owner path");
        let mut restored = must_restore(&compiled, &bytes, policy());
        assert_same(&original, &restored);
        let initialize = || {
            let builder = must(
                compiled
                    .input_snapshot()
                    .pulse(ExternalInputKey::<Pulse>::from_u128(0x71), PulseCount::ONE),
            );
            must(builder.finish())
        };
        must(original.apply(Transaction::initialize(
            Time::from_ticks(1),
            original.revision(),
            initialize(),
        )));
        must(restored.apply(Transaction::initialize(
            Time::from_ticks(1),
            restored.revision(),
            initialize(),
        )));
        assert_same(&original, &restored);
        let again = must_restore(&compiled, &encoded(&original), policy());
        assert_same(&original, &again);
    }
}
