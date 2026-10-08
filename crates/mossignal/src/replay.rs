//! Deterministic replay over the ordinary transaction transition.
//!
//! Chosen artifact keys, recorded here and not a format freeze:
//! input payloads use `network_key`, `network_fingerprint`,
//! `input_schema_fingerprint`, `levels`, and `pulses`. A transaction record
//! uses `requested_time`, `expected_revision`, those same bindings, and `input`.
//! A frame uses `frame_index`, `expected_previous_execution_digest`,
//! `expected_revision`, `runtime_policy_id`, `transaction_record`,
//! `resulting_execution_digest`, and `resulting_observable_digest`. A log uses
//! `time_domain_id`, `network_key`, `network_fingerprint`, `starting_revision`,
//! `starting_execution_digest`, `starting_observable_digest`,
//! `runtime_policy_id`, `semantic_versions`, `frames`, `final_revision`,
//! `final_execution_digest`, and `final_observable_digest`.
//! Level and pulse observations are separate key-sorted arrays. Pulse counts
//! are encoded only when positive. `frame_index` is the position in that log,
//! from zero. Concatenation renumbers it and does not keep a second index.
//! The log content digest is BLAKE3 over `mossignal/replay_log_content/v2`,
//! and each frame's contribution is that frame's `artifact_integrity`
//! digest. Presentation metadata is omitted.

#![allow(clippy::result_large_err)]

use crate::cbor_decode::{self, DecodeError, Limits, Value};
use crate::diagnostics::{
    BudgetEvidence, CanonicalEncodingEvidence, DiagnosticCode, DigestMismatchEvidence,
    OperationSubjectRef, Problem, ProblemEvidence, ReplayEvidence, Responsibility, Severity,
    SubjectRef, VersionCompatibilityEvidence,
};
use crate::identity::{
    Cbor, ExecutionStateDigest, InputSchemaFingerprint, NetworkFingerprint, ObservableStateDigest,
    TimeDomainId, domain_separated,
};
use crate::input::{InputDelta, InputSnapshot};
use crate::key::{ExternalInputKey, NetworkKey};
use crate::machine::{Machine, NetworkRevision};
use crate::persistence::{ArtifactBytes, EncodeFailure, PersistenceContext};
use crate::policy::RuntimePolicyId;
use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
use crate::snapshot_restore::DecodePolicy;
use crate::transaction::{RuntimeFailure, Transaction, TransactionResult};
use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;

const ARTIFACT_PREFIX: [u8; 8] = [0x4d, 0x53, 0x49, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const VERSION: u64 = 2;
const ARTIFACT_INTEGRITY_DOMAIN: &str = "mossignal/artifact_integrity/v2";
const REPLAY_LOG_CONTENT_DOMAIN: &str = "mossignal/replay_log_content/v2";

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

const VERSION_FIELDS: &[&str] = &[
    "artifact_schema_version",
    "canonical_encoding_version",
    "core_semantics_version",
    "diagnostic_schema_version",
    "digest_suite_version",
    "envelope_schema_version",
    "node_semantics_version",
    "patch_semantics_version",
    "provenance_semantics_version",
];

const INPUT_FIELDS: &[&str] = &[
    "input_schema_fingerprint",
    "levels",
    "network_fingerprint",
    "network_key",
    "pulses",
];

const TRANSACTION_FIELDS: &[&str] = &[
    "expected_revision",
    "input",
    "input_schema_fingerprint",
    "network_fingerprint",
    "network_key",
    "patch",
    "requested_time",
];

const FRAME_FIELDS: &[&str] = &[
    "expected_previous_execution_digest",
    "expected_revision",
    "frame_index",
    "resulting_execution_digest",
    "resulting_observable_digest",
    "runtime_policy_id",
    "transaction_record",
    "transaction_result",
];

const LOG_FIELDS: &[&str] = &[
    "final_execution_digest",
    "final_observable_digest",
    "final_revision",
    "frames",
    "network_fingerprint",
    "network_key",
    "runtime_policy_id",
    "semantic_versions",
    "starting_execution_digest",
    "starting_observable_digest",
    "starting_revision",
    "time_domain_id",
];

/// Opaque digest of one replay log's canonical content.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReplayLogContentDigest([u8; 32]);

impl ReplayLogContentDigest {
    /// Returns the digest bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    const fn from_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for ReplayLogContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ReplayLogContentDigest({})", hex(&self.0))
    }
}

impl fmt::Display for ReplayLogContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex(&self.0))
    }
}

/// One recorded transaction and the digests the ordinary transition published.
pub struct ReplayFrame<D> {
    frame_index: u64,
    time_domain: TimeDomainId,
    expected_previous_execution_digest: ExecutionStateDigest,
    expected_revision: NetworkRevision,
    runtime_policy_id: RuntimePolicyId,
    transaction: Transaction<D>,
    resulting_execution_digest: ExecutionStateDigest,
    resulting_observable_digest: ObservableStateDigest,
}

impl<D> Clone for ReplayFrame<D> {
    fn clone(&self) -> Self {
        Self {
            frame_index: self.frame_index,
            time_domain: self.time_domain,
            expected_previous_execution_digest: self.expected_previous_execution_digest,
            expected_revision: self.expected_revision,
            runtime_policy_id: self.runtime_policy_id,
            transaction: self.transaction.clone(),
            resulting_execution_digest: self.resulting_execution_digest,
            resulting_observable_digest: self.resulting_observable_digest,
        }
    }
}

impl<D> ReplayFrame<D> {
    /// Builds a frame from a transaction and the result `apply` published for it.
    ///
    /// `runtime_policy_id` is supplied by the caller because a transaction
    /// result does not carry the policy that governed the transition. The
    /// frame index is zero until a log assigns positions.
    pub fn from_success(
        transaction: Transaction<D>,
        result: &TransactionResult<D>,
        runtime_policy_id: RuntimePolicyId,
        time_domain: TimeDomainId,
    ) -> Result<Self, ReplayFailure<D>> {
        if transaction.carries_patch() {
            let mut evidence = ReplayEvidence::new();
            evidence.underlying_code = "patch".to_owned();
            return Err(replay_fail(
                ReplayVariant::PatchPreparationDiverged,
                EvidenceVariant::PatchPreparationDiverged,
                evidence,
            ));
        }
        if transaction.expected_revision() != result.before_revision() {
            let mut evidence = ReplayEvidence::new();
            evidence.expected_revision = Some(transaction.expected_revision().value());
            evidence.actual_revision = Some(result.before_revision().value());
            evidence.logical_time = Some(transaction.requested_time().ticks());
            return Err(replay_fail(
                ReplayVariant::ExpectedRevisionMismatch,
                EvidenceVariant::ExpectedRevisionMismatch,
                evidence,
            ));
        }
        Ok(Self {
            frame_index: 0,
            time_domain,
            expected_previous_execution_digest: result.before_execution_digest(),
            expected_revision: result.before_revision(),
            runtime_policy_id,
            transaction,
            resulting_execution_digest: result.after_execution_digest(),
            resulting_observable_digest: result.after_observable_digest(),
        })
    }

    /// Returns this frame's position in its log.
    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Returns the execution digest required before this frame is applied.
    #[must_use]
    pub const fn expected_previous_execution_digest(&self) -> ExecutionStateDigest {
        self.expected_previous_execution_digest
    }

    /// Returns the topology revision this frame expects.
    #[must_use]
    pub const fn expected_revision(&self) -> NetworkRevision {
        self.expected_revision
    }

    /// Returns the runtime policy this frame was recorded under.
    #[must_use]
    pub const fn runtime_policy_id(&self) -> RuntimePolicyId {
        self.runtime_policy_id
    }

    /// Returns the transaction this frame will apply.
    #[must_use]
    pub const fn transaction(&self) -> &Transaction<D> {
        &self.transaction
    }

    /// Returns the execution digest published when this frame succeeds.
    #[must_use]
    pub const fn resulting_execution_digest(&self) -> ExecutionStateDigest {
        self.resulting_execution_digest
    }

    /// Returns the observable digest published when this frame succeeds.
    #[must_use]
    pub const fn resulting_observable_digest(&self) -> ObservableStateDigest {
        self.resulting_observable_digest
    }

    /// Returns the time domain carried by this frame's artifact envelope.
    #[must_use]
    pub const fn time_domain(&self) -> TimeDomainId {
        self.time_domain
    }
}

impl<D> fmt::Debug for ReplayFrame<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayFrame")
            .field("frame_index", &self.frame_index)
            .field("expected_revision", &self.expected_revision.value())
            .field("requested_time", &self.transaction.requested_time().ticks())
            .finish()
    }
}

/// The result and owned frame of one successful recorded transition.
pub struct RecordedTransaction<D> {
    result: TransactionResult<D>,
    frame: ReplayFrame<D>,
}

impl<D> RecordedTransaction<D> {
    /// Returns the result `apply` published.
    #[must_use]
    pub const fn result(&self) -> &TransactionResult<D> {
        &self.result
    }

    /// Returns the frame that records that result.
    #[must_use]
    pub const fn frame(&self) -> &ReplayFrame<D> {
        &self.frame
    }

    /// Returns the published result.
    #[must_use]
    pub fn into_result(self) -> TransactionResult<D> {
        self.result
    }
}

impl<D> fmt::Debug for RecordedTransaction<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordedTransaction")
            .field("frame", &self.frame)
            .finish()
    }
}

/// Version vector stored with a replay log.
///
/// Every component matches the machine-snapshot encoder's current vector.
/// Accepting only that vector is not a replay-format freeze.
#[derive(Clone, Copy, PartialEq, Eq)]
struct SemanticVersions {
    artifact_schema_version: u64,
    canonical_encoding_version: u64,
    core_semantics_version: u64,
    diagnostic_schema_version: u64,
    digest_suite_version: u64,
    envelope_schema_version: u64,
    node_semantics_version: u64,
    patch_semantics_version: u64,
    provenance_semantics_version: u64,
}

impl SemanticVersions {
    const CURRENT: Self = Self {
        artifact_schema_version: VERSION,
        canonical_encoding_version: VERSION,
        core_semantics_version: VERSION,
        diagnostic_schema_version: VERSION,
        digest_suite_version: VERSION,
        envelope_schema_version: VERSION,
        node_semantics_version: VERSION,
        patch_semantics_version: VERSION,
        provenance_semantics_version: VERSION,
    };

    fn component(self, name: &str) -> Option<u64> {
        Some(match name {
            "artifact_schema_version" => self.artifact_schema_version,
            "canonical_encoding_version" => self.canonical_encoding_version,
            "core_semantics_version" => self.core_semantics_version,
            "diagnostic_schema_version" => self.diagnostic_schema_version,
            "digest_suite_version" => self.digest_suite_version,
            "envelope_schema_version" => self.envelope_schema_version,
            "node_semantics_version" => self.node_semantics_version,
            "patch_semantics_version" => self.patch_semantics_version,
            "provenance_semantics_version" => self.provenance_semantics_version,
            _ => return None,
        })
    }

    fn first_difference(self, other: Self) -> Option<(&'static str, u64, u64)> {
        VERSION_FIELDS.iter().find_map(|name| {
            let left = self.component(name)?;
            let right = other.component(name)?;
            (left != right).then_some((*name, left, right))
        })
    }
}

/// One ordered replay of committed transactions from a checked starting machine.
pub struct ReplayLog<D> {
    time_domain: TimeDomainId,
    network_key: NetworkKey,
    network_fingerprint: NetworkFingerprint,
    starting_revision: NetworkRevision,
    starting_execution_digest: ExecutionStateDigest,
    starting_observable_digest: ObservableStateDigest,
    runtime_policy_id: RuntimePolicyId,
    versions: SemanticVersions,
    frames: Vec<ReplayFrame<D>>,
    final_execution_digest: ExecutionStateDigest,
    final_observable_digest: ObservableStateDigest,
    final_revision: NetworkRevision,
    content_digest: ReplayLogContentDigest,
}

impl<D> Clone for ReplayLog<D> {
    fn clone(&self) -> Self {
        Self {
            time_domain: self.time_domain,
            network_key: self.network_key,
            network_fingerprint: self.network_fingerprint,
            starting_revision: self.starting_revision,
            starting_execution_digest: self.starting_execution_digest,
            starting_observable_digest: self.starting_observable_digest,
            runtime_policy_id: self.runtime_policy_id,
            versions: self.versions,
            frames: self.frames.clone(),
            final_execution_digest: self.final_execution_digest,
            final_observable_digest: self.final_observable_digest,
            final_revision: self.final_revision,
            content_digest: self.content_digest,
        }
    }
}

impl<D> ReplayLog<D> {
    /// Returns the content digest of this log.
    #[must_use]
    pub const fn content_digest(&self) -> ReplayLogContentDigest {
        self.content_digest
    }

    /// Returns the frames in semantic order.
    #[must_use]
    pub fn frames(&self) -> &[ReplayFrame<D>] {
        &self.frames
    }

    /// Returns the log time domain.
    #[must_use]
    pub const fn time_domain(&self) -> TimeDomainId {
        self.time_domain
    }

    /// Returns the starting network key.
    #[must_use]
    pub const fn network_key(&self) -> NetworkKey {
        self.network_key
    }

    /// Returns the starting network fingerprint.
    #[must_use]
    pub const fn network_fingerprint(&self) -> NetworkFingerprint {
        self.network_fingerprint
    }

    /// Returns the starting topology revision.
    #[must_use]
    pub const fn starting_revision(&self) -> NetworkRevision {
        self.starting_revision
    }

    /// Returns the starting execution digest.
    #[must_use]
    pub const fn starting_execution_digest(&self) -> ExecutionStateDigest {
        self.starting_execution_digest
    }

    /// Returns the starting observable digest.
    #[must_use]
    pub const fn starting_observable_digest(&self) -> ObservableStateDigest {
        self.starting_observable_digest
    }

    /// Returns the runtime policy identity required by every frame.
    #[must_use]
    pub const fn runtime_policy_id(&self) -> RuntimePolicyId {
        self.runtime_policy_id
    }

    /// Returns the execution digest expected after the last frame.
    #[must_use]
    pub const fn final_execution_digest(&self) -> ExecutionStateDigest {
        self.final_execution_digest
    }

    /// Returns the observable digest expected after the last frame.
    #[must_use]
    pub const fn final_observable_digest(&self) -> ObservableStateDigest {
        self.final_observable_digest
    }

    /// Returns the topology revision expected after the last frame.
    #[must_use]
    pub const fn final_revision(&self) -> NetworkRevision {
        self.final_revision
    }

    /// Appends `next` when the boundary identities match.
    ///
    /// The result numbers `frame_index` from zero across the combined sequence.
    /// Renumbering changes each moved frame's bytes and integrity digest.
    pub fn concatenate(&self, next: &Self) -> Result<Self, ReplayFailure<D>> {
        if let Some(failure) = concatenation_failure(self, next) {
            return Err(failure);
        }
        let mut frames = Vec::with_capacity(self.frames.len() + next.frames.len());
        for (position, frame) in self.frames.iter().chain(next.frames.iter()).enumerate() {
            let mut frame = frame.clone();
            frame.frame_index = position_index(position);
            frames.push(frame);
        }
        Ok(assemble_log(
            Checkpoint {
                time_domain: self.time_domain,
                network_key: self.network_key,
                network_fingerprint: self.network_fingerprint,
                revision: self.starting_revision,
                execution: self.starting_execution_digest,
                observable: self.starting_observable_digest,
                policy: self.runtime_policy_id,
            },
            frames,
            Checkpoint {
                time_domain: next.time_domain,
                network_key: next.network_key,
                network_fingerprint: next.network_fingerprint,
                revision: next.final_revision,
                execution: next.final_execution_digest,
                observable: next.final_observable_digest,
                policy: next.runtime_policy_id,
            },
            self.versions,
        ))
    }
}

impl<D> fmt::Debug for ReplayLog<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayLog")
            .field("frames", &self.frames.len())
            .field("content_digest", &self.content_digest)
            .finish()
    }
}

/// Failure while decoding, concatenating, or replaying.
///
/// [`ReplayFailure::Decode`] keeps the persistence code of a framing or version
/// failure. [`ReplayFailure::Underlying`] keeps the runtime code of a failed
/// `apply` and carries the frame index only as location context.
/// [`ReplayFailure::Materialization`] keeps the input-construction code when a
/// frame's observations do not fit the live schema.
#[non_exhaustive]
pub enum ReplayFailure<D> {
    /// The artifact bytes were not a supported canonical replay artifact.
    Decode(crate::DecodeFailure<D>),
    /// The frame's observations were rejected by the live input builder.
    Materialization {
        /// Catalogue problem produced by input construction.
        problem: Problem<D>,
        /// Frame that could not be materialized.
        frame_index: u64,
        /// Requested logical time of that frame.
        logical_time: Option<u64>,
    },
    /// `apply` rejected the frame and published nothing for it.
    Underlying {
        /// Runtime rejection, unchanged.
        failure: RuntimeFailure<D>,
        /// Frame that `apply` rejected.
        frame_index: u64,
        /// Requested logical time of that frame.
        logical_time: Option<u64>,
    },
    /// The machine execution digest is not the log's starting digest.
    StartingExecutionDigestMismatch(Problem<D>),
    /// The machine observable digest is not the log's starting digest.
    StartingObservableDigestMismatch(Problem<D>),
    /// The machine revision is not the revision this checkpoint expects.
    ExpectedRevisionMismatch(Problem<D>),
    /// The machine policy is not the log or frame policy.
    RuntimePolicyMismatch(Problem<D>),
    /// The machine time domain is not the log time domain.
    TimeDomainMismatch(Problem<D>),
    /// The machine fingerprint is not the log or frame fingerprint.
    NetworkFingerprintMismatch(Problem<D>),
    /// Two logs do not meet at a shared checkpoint.
    LogsNotConcatenable(Problem<D>),
    /// A frame carries a patch attachment this replay does not execute.
    PatchPreparationDiverged(Problem<D>),
    /// A successful apply published a different execution digest.
    ResultingExecutionDigestMismatch(Problem<D>),
    /// A successful apply published a different observable digest.
    ResultingObservableDigestMismatch(Problem<D>),
    /// The frame sequence skips a position.
    FrameMissing(Problem<D>),
    /// The frame sequence contradicts order or the prior execution digest.
    FrameReordered(Problem<D>),
    /// The same frame position occurs twice.
    FrameDuplicated(Problem<D>),
}

impl<D> ReplayFailure<D> {
    /// Returns the catalogue code for this leaf.
    #[must_use]
    pub fn code(&self) -> DiagnosticCode {
        self.problem().code()
    }

    /// Returns the catalogue severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code().severity()
    }

    /// Returns the catalogue responsibility.
    #[must_use]
    pub fn responsibility(&self) -> Responsibility {
        self.code().responsibility()
    }

    /// Returns the catalogue problem.
    #[must_use]
    pub fn problem(&self) -> &Problem<D> {
        match self {
            Self::Decode(failure) => failure.problem(),
            Self::Materialization { problem, .. } => problem,
            Self::Underlying { failure, .. } => failure.problem(),
            Self::StartingExecutionDigestMismatch(problem)
            | Self::StartingObservableDigestMismatch(problem)
            | Self::ExpectedRevisionMismatch(problem)
            | Self::RuntimePolicyMismatch(problem)
            | Self::TimeDomainMismatch(problem)
            | Self::NetworkFingerprintMismatch(problem)
            | Self::LogsNotConcatenable(problem)
            | Self::PatchPreparationDiverged(problem)
            | Self::ResultingExecutionDigestMismatch(problem)
            | Self::ResultingObservableDigestMismatch(problem)
            | Self::FrameMissing(problem)
            | Self::FrameReordered(problem)
            | Self::FrameDuplicated(problem) => problem,
        }
    }

    /// Returns the frame index when this failure names one frame.
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        match self {
            Self::Decode(_) => None,
            Self::Materialization { frame_index, .. } | Self::Underlying { frame_index, .. } => {
                Some(*frame_index)
            }
            _ => replay_evidence_of(self.problem()).and_then(|evidence| evidence.frame_index),
        }
    }

    /// Returns the logical time when this failure names one frame.
    #[must_use]
    pub fn logical_time(&self) -> Option<u64> {
        match self {
            Self::Decode(_) => None,
            Self::Materialization { logical_time, .. } | Self::Underlying { logical_time, .. } => {
                *logical_time
            }
            _ => replay_evidence_of(self.problem()).and_then(|evidence| evidence.logical_time),
        }
    }
}

impl<D> fmt::Debug for ReplayFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayFailure")
            .field("code", &self.code().as_str())
            .field("frame_index", &self.frame_index())
            .finish()
    }
}

impl<D> fmt::Display for ReplayFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "replay failed with {}", self.code().as_str())
    }
}

impl<D> std::error::Error for ReplayFailure<D> {}

impl<D> From<crate::DecodeFailure<D>> for ReplayFailure<D> {
    fn from(failure: crate::DecodeFailure<D>) -> Self {
        Self::Decode(failure)
    }
}

impl<D> Machine<D> {
    /// Applies `transaction` and returns the result with its replay frame.
    ///
    /// The transition is [`Machine::apply`]. The clone exists so the frame can
    /// retain the transaction after `apply` consumes it.
    pub fn apply_recorded(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<RecordedTransaction<D>, RuntimeFailure<D>> {
        if transaction.carries_patch() {
            return Err(RuntimeFailure::rejected_patch_recording());
        }
        let retained = transaction.clone();
        let policy_id = self.runtime_policy_id();
        let time_domain = self.compiled().time_domain_id();
        let result = self.apply(transaction)?;
        let frame = match ReplayFrame::from_success(retained, &result, policy_id, time_domain) {
            Ok(frame) => frame,
            Err(failure) => {
                panic!(
                    "successful apply retains the transaction revision and execution digest: {failure}"
                )
            }
        };
        Ok(RecordedTransaction { result, frame })
    }

    /// Replays `frames` in order through [`Machine::apply`].
    ///
    /// A failed apply leaves the machine as that transaction left it. A
    /// successful apply whose resulting digest disagrees also leaves the
    /// published machine in place and stops the sequence.
    pub fn replay(
        &mut self,
        frames: &[ReplayFrame<D>],
    ) -> Result<Vec<TransactionResult<D>>, ReplayFailure<D>> {
        apply_sequence(self, frames, SequenceOrigin::Frames, "")
    }

    /// Replays `log` after the log-to-machine checkpoint matches.
    pub fn replay_log(
        &mut self,
        log: &ReplayLog<D>,
    ) -> Result<Vec<TransactionResult<D>>, ReplayFailure<D>> {
        check_log_boundary(self, log)?;
        check_log_linkage(log)?;
        let results = apply_sequence(
            self,
            &log.frames,
            SequenceOrigin::Log,
            &hex(&log.content_digest.as_bytes()),
        )?;
        check_log_finals(self, log)?;
        Ok(results)
    }
}

/// Records `transactions` by applying each one, and returns their log.
///
/// A failed transaction returns that runtime failure. Earlier transactions in
/// the iterator stay published, and no log is returned.
pub fn record_replay_log<D>(
    machine: &mut Machine<D>,
    transactions: impl IntoIterator<Item = Transaction<D>>,
) -> Result<ReplayLog<D>, RuntimeFailure<D>> {
    let starting = checkpoint(machine);
    let mut frames = Vec::new();
    for transaction in transactions {
        frames.push(machine.apply_recorded(transaction)?.frame);
    }
    for (position, frame) in frames.iter_mut().enumerate() {
        frame.frame_index = position_index(position);
    }
    let ending = checkpoint(machine);
    Ok(assemble_log(
        starting,
        frames,
        ending,
        SemanticVersions::CURRENT,
    ))
}

/// Encodes `frame` as one standalone `replay_frame` artifact.
pub fn encode_replay_frame<D>(
    context: &PersistenceContext<D>,
    frame: &ReplayFrame<D>,
) -> Result<ArtifactBytes, EncodeFailure> {
    if context.time_domain() != frame.time_domain {
        return Err(EncodeFailure::TimeDomainMismatch);
    }
    Ok(ArtifactBytes::from_canonical(encoded_frame(frame).bytes))
}

/// Encodes `log` as one standalone `replay_log` artifact.
pub fn encode_replay_log<D>(
    context: &PersistenceContext<D>,
    log: &ReplayLog<D>,
) -> Result<ArtifactBytes, EncodeFailure> {
    if context.time_domain() != log.time_domain {
        return Err(EncodeFailure::TimeDomainMismatch);
    }
    for frame in &log.frames {
        if frame.time_domain != log.time_domain {
            return Err(EncodeFailure::TimeDomainMismatch);
        }
    }
    Ok(ArtifactBytes::from_canonical(encoded_log(log).bytes))
}

/// Decodes one standalone `replay_frame` artifact.
pub fn decode_replay_frame<D>(
    context: &PersistenceContext<D>,
    bytes: &[u8],
    policy: &DecodePolicy,
) -> Result<ReplayFrame<D>, ReplayFailure<D>> {
    if policy.replay_frames() == 0 {
        return Err(ReplayFailure::Decode(limit_failure(
            "replay_frame",
            "replay_frames",
            policy.replay_frames(),
            1,
        )));
    }
    let opened = open_kind(bytes, policy, "replay_frame")?;
    if opened.time_domain != context.time_domain() {
        return Err(ReplayFailure::Decode(wrong_time_domain(
            "replay_frame",
            "context",
            &hex(&context.time_domain().to_be_bytes()),
            &hex(&opened.time_domain.to_be_bytes()),
        )));
    }
    parse_frame(opened, policy).map(|(frame, _integrity)| frame)
}

/// Decodes one standalone `replay_log` artifact.
pub fn decode_replay_log<D>(
    context: &PersistenceContext<D>,
    bytes: &[u8],
    policy: &DecodePolicy,
) -> Result<ReplayLog<D>, ReplayFailure<D>> {
    let opened = open_kind(bytes, policy, "replay_log")?;
    if opened.time_domain != context.time_domain() {
        return Err(ReplayFailure::Decode(wrong_time_domain(
            "replay_log",
            "context",
            &hex(&context.time_domain().to_be_bytes()),
            &hex(&opened.time_domain.to_be_bytes()),
        )));
    }
    parse_log(opened, policy)
}

#[derive(Clone, Copy)]
struct Checkpoint {
    time_domain: TimeDomainId,
    network_key: NetworkKey,
    network_fingerprint: NetworkFingerprint,
    revision: NetworkRevision,
    execution: ExecutionStateDigest,
    observable: ObservableStateDigest,
    policy: RuntimePolicyId,
}

fn checkpoint<D>(machine: &Machine<D>) -> Checkpoint {
    Checkpoint {
        time_domain: machine.compiled().time_domain_id(),
        network_key: machine.compiled().network_key(),
        network_fingerprint: machine.fingerprint(),
        revision: machine.revision(),
        execution: machine.execution_state_digest(),
        observable: machine.observable_state_digest(),
        policy: machine.runtime_policy_id(),
    }
}

fn assemble_log<D>(
    starting: Checkpoint,
    frames: Vec<ReplayFrame<D>>,
    ending: Checkpoint,
    versions: SemanticVersions,
) -> ReplayLog<D> {
    let integrity = frames
        .iter()
        .map(|frame| encoded_frame(frame).integrity)
        .collect::<Vec<_>>();
    let content_digest = log_content_digest(&starting, &ending, &versions, &integrity);
    ReplayLog {
        time_domain: starting.time_domain,
        network_key: starting.network_key,
        network_fingerprint: starting.network_fingerprint,
        starting_revision: starting.revision,
        starting_execution_digest: starting.execution,
        starting_observable_digest: starting.observable,
        runtime_policy_id: starting.policy,
        versions,
        frames,
        final_execution_digest: ending.execution,
        final_observable_digest: ending.observable,
        final_revision: ending.revision,
        content_digest,
    }
}

fn position_index(position: usize) -> u64 {
    u64::try_from(position).unwrap_or(u64::MAX)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SequenceOrigin {
    Frames,
    Log,
}

fn apply_sequence<D>(
    machine: &mut Machine<D>,
    frames: &[ReplayFrame<D>],
    origin: SequenceOrigin,
    log_id: &str,
) -> Result<Vec<TransactionResult<D>>, ReplayFailure<D>> {
    let mut results = Vec::with_capacity(frames.len());
    for (position, frame) in frames.iter().enumerate() {
        if let Some(failure) = index_failure(frames, position, log_id) {
            return Err(failure);
        }
        let mut evidence = frame_evidence(frame, log_id);
        if machine.execution_state_digest() != frame.expected_previous_execution_digest {
            evidence.expected_execution_digest =
                hex(&frame.expected_previous_execution_digest.as_bytes());
            evidence.actual_execution_digest = hex(&machine.execution_state_digest().as_bytes());
            let failure = if position == 0 && origin == SequenceOrigin::Frames {
                replay_fail(
                    ReplayVariant::StartingExecutionDigestMismatch,
                    EvidenceVariant::StartingExecutionDigestMismatch,
                    evidence,
                )
            } else {
                replay_fail(
                    ReplayVariant::FrameReordered,
                    EvidenceVariant::FrameReordered,
                    evidence,
                )
            };
            return Err(failure);
        }
        if machine.revision() != frame.expected_revision {
            evidence.expected_revision = Some(frame.expected_revision.value());
            evidence.actual_revision = Some(machine.revision().value());
            return Err(replay_fail(
                ReplayVariant::ExpectedRevisionMismatch,
                EvidenceVariant::ExpectedRevisionMismatch,
                evidence,
            ));
        }
        if machine.runtime_policy_id() != frame.runtime_policy_id {
            evidence.expected_policy = hex(&frame.runtime_policy_id.as_bytes());
            evidence.actual_policy = hex(&machine.runtime_policy_id().as_bytes());
            return Err(replay_fail(
                ReplayVariant::RuntimePolicyMismatch,
                EvidenceVariant::RuntimePolicyMismatch,
                evidence,
            ));
        }
        let transaction = rematerialize(machine, frame, &evidence)?;
        // SPEC: docs/specs/contracts/replay-artifacts.yaml "ordinary-fold"
        // Replay publishes only through the ordinary transaction transition.
        let result = match machine.apply(transaction) {
            Ok(result) => result,
            Err(failure) => {
                return Err(ReplayFailure::Underlying {
                    failure,
                    frame_index: frame.frame_index,
                    logical_time: evidence.logical_time,
                });
            }
        };
        if result.after_execution_digest() != frame.resulting_execution_digest {
            evidence.expected_execution_digest = hex(&frame.resulting_execution_digest.as_bytes());
            evidence.actual_execution_digest = hex(&result.after_execution_digest().as_bytes());
            // SPEC: docs/specs/contracts/replay-artifacts.yaml "chain-and-localization"
            // A digest mismatch after a successful apply keeps the published machine.
            return Err(replay_fail(
                ReplayVariant::ResultingExecutionDigestMismatch,
                EvidenceVariant::ResultingExecutionDigestMismatch,
                evidence,
            ));
        }
        if result.after_observable_digest() != frame.resulting_observable_digest {
            evidence.expected_observable_digest =
                hex(&frame.resulting_observable_digest.as_bytes());
            evidence.actual_observable_digest = hex(&result.after_observable_digest().as_bytes());
            return Err(replay_fail(
                ReplayVariant::ResultingObservableDigestMismatch,
                EvidenceVariant::ResultingObservableDigestMismatch,
                evidence,
            ));
        }
        results.push(result);
    }
    Ok(results)
}

fn index_failure<D>(
    frames: &[ReplayFrame<D>],
    position: usize,
    log_id: &str,
) -> Option<ReplayFailure<D>> {
    let frame = &frames[position];
    let expected = position_index(position);
    let evidence = frame_evidence(frame, log_id);
    if frames[..position]
        .iter()
        .any(|earlier| earlier.frame_index == frame.frame_index)
    {
        return Some(replay_fail(
            ReplayVariant::FrameDuplicated,
            EvidenceVariant::FrameDuplicated,
            evidence,
        ));
    }
    if frame.frame_index == expected {
        return None;
    }
    let expected_later = frames[position + 1..]
        .iter()
        .any(|later| later.frame_index == expected);
    if frame.frame_index < expected || expected_later {
        Some(replay_fail(
            ReplayVariant::FrameReordered,
            EvidenceVariant::FrameReordered,
            evidence,
        ))
    } else {
        Some(replay_fail(
            ReplayVariant::FrameMissing,
            EvidenceVariant::FrameMissing,
            evidence,
        ))
    }
}

fn check_log_boundary<D>(machine: &Machine<D>, log: &ReplayLog<D>) -> Result<(), ReplayFailure<D>> {
    let mut evidence = ReplayEvidence::new();
    evidence.log_id = hex(&log.content_digest.as_bytes());
    if machine.compiled().time_domain_id() != log.time_domain {
        evidence.expected_time_domain = hex(&log.time_domain.to_be_bytes());
        evidence.actual_time_domain = hex(&machine.compiled().time_domain_id().to_be_bytes());
        return Err(replay_fail(
            ReplayVariant::TimeDomainMismatch,
            EvidenceVariant::TimeDomainMismatch,
            evidence,
        ));
    }
    if machine.fingerprint() != log.network_fingerprint {
        evidence.expected_fingerprint = hex(&log.network_fingerprint.as_bytes());
        evidence.actual_fingerprint = hex(&machine.fingerprint().as_bytes());
        return Err(replay_fail(
            ReplayVariant::NetworkFingerprintMismatch,
            EvidenceVariant::NetworkFingerprintMismatch,
            evidence,
        ));
    }
    if machine.runtime_policy_id() != log.runtime_policy_id {
        evidence.expected_policy = hex(&log.runtime_policy_id.as_bytes());
        evidence.actual_policy = hex(&machine.runtime_policy_id().as_bytes());
        return Err(replay_fail(
            ReplayVariant::RuntimePolicyMismatch,
            EvidenceVariant::RuntimePolicyMismatch,
            evidence,
        ));
    }
    if machine.revision() != log.starting_revision {
        evidence.expected_revision = Some(log.starting_revision.value());
        evidence.actual_revision = Some(machine.revision().value());
        return Err(replay_fail(
            ReplayVariant::ExpectedRevisionMismatch,
            EvidenceVariant::ExpectedRevisionMismatch,
            evidence,
        ));
    }
    if machine.execution_state_digest() != log.starting_execution_digest {
        evidence.expected_execution_digest = hex(&log.starting_execution_digest.as_bytes());
        evidence.actual_execution_digest = hex(&machine.execution_state_digest().as_bytes());
        return Err(replay_fail(
            ReplayVariant::StartingExecutionDigestMismatch,
            EvidenceVariant::StartingExecutionDigestMismatch,
            evidence,
        ));
    }
    if machine.observable_state_digest() != log.starting_observable_digest {
        evidence.expected_observable_digest = hex(&log.starting_observable_digest.as_bytes());
        evidence.actual_observable_digest = hex(&machine.observable_state_digest().as_bytes());
        return Err(replay_fail(
            ReplayVariant::StartingObservableDigestMismatch,
            EvidenceVariant::StartingObservableDigestMismatch,
            evidence,
        ));
    }
    Ok(())
}

fn check_log_linkage<D>(log: &ReplayLog<D>) -> Result<(), ReplayFailure<D>> {
    let Some(frame) = log.frames.first() else {
        return Ok(());
    };
    let digest_disagrees =
        frame.expected_previous_execution_digest != log.starting_execution_digest;
    let revision_disagrees = frame.expected_revision != log.starting_revision;
    if !digest_disagrees && !revision_disagrees {
        return Ok(());
    }
    let mut evidence = frame_evidence(frame, &hex(&log.content_digest.as_bytes()));
    evidence.expected_execution_digest = hex(&frame.expected_previous_execution_digest.as_bytes());
    evidence.actual_execution_digest = hex(&log.starting_execution_digest.as_bytes());
    evidence.expected_revision = Some(frame.expected_revision.value());
    evidence.actual_revision = Some(log.starting_revision.value());
    Err(replay_fail(
        ReplayVariant::FrameReordered,
        EvidenceVariant::FrameReordered,
        evidence,
    ))
}

fn check_log_finals<D>(machine: &Machine<D>, log: &ReplayLog<D>) -> Result<(), ReplayFailure<D>> {
    let mut evidence = ReplayEvidence::new();
    evidence.log_id = hex(&log.content_digest.as_bytes());
    if let Some(frame) = log.frames.last() {
        evidence.frame_index = Some(frame.frame_index);
        evidence.frame_id = frame.frame_index.to_string();
        evidence.logical_time = Some(frame.transaction.requested_time().ticks());
    }
    if machine.execution_state_digest() != log.final_execution_digest {
        evidence.expected_execution_digest = hex(&log.final_execution_digest.as_bytes());
        evidence.actual_execution_digest = hex(&machine.execution_state_digest().as_bytes());
        return Err(replay_fail(
            ReplayVariant::ResultingExecutionDigestMismatch,
            EvidenceVariant::ResultingExecutionDigestMismatch,
            evidence,
        ));
    }
    if machine.observable_state_digest() != log.final_observable_digest {
        evidence.expected_observable_digest = hex(&log.final_observable_digest.as_bytes());
        evidence.actual_observable_digest = hex(&machine.observable_state_digest().as_bytes());
        return Err(replay_fail(
            ReplayVariant::ResultingObservableDigestMismatch,
            EvidenceVariant::ResultingObservableDigestMismatch,
            evidence,
        ));
    }
    if machine.revision() != log.final_revision {
        evidence.expected_revision = Some(log.final_revision.value());
        evidence.actual_revision = Some(machine.revision().value());
        return Err(replay_fail(
            ReplayVariant::ExpectedRevisionMismatch,
            EvidenceVariant::ExpectedRevisionMismatch,
            evidence,
        ));
    }
    Ok(())
}

fn concatenation_failure<D>(left: &ReplayLog<D>, right: &ReplayLog<D>) -> Option<ReplayFailure<D>> {
    let mut evidence = ReplayEvidence::new();
    evidence.log_id = hex(&left.content_digest.as_bytes());
    evidence.frame_id = hex(&right.content_digest.as_bytes());
    if left.time_domain != right.time_domain {
        evidence.expected_time_domain = hex(&left.time_domain.to_be_bytes());
        evidence.actual_time_domain = hex(&right.time_domain.to_be_bytes());
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    if left.network_fingerprint != right.network_fingerprint
        || left.network_key != right.network_key
    {
        evidence.expected_fingerprint = hex(&left.network_fingerprint.as_bytes());
        evidence.actual_fingerprint = hex(&right.network_fingerprint.as_bytes());
        evidence.underlying_code = "network".to_owned();
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    if left.runtime_policy_id != right.runtime_policy_id {
        evidence.expected_policy = hex(&left.runtime_policy_id.as_bytes());
        evidence.actual_policy = hex(&right.runtime_policy_id.as_bytes());
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    if let Some((name, expected, actual)) = left.versions.first_difference(right.versions) {
        evidence.underlying_code = name.to_owned();
        evidence.expected_revision = Some(expected);
        evidence.actual_revision = Some(actual);
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    if left.final_execution_digest != right.starting_execution_digest {
        evidence.expected_execution_digest = hex(&left.final_execution_digest.as_bytes());
        evidence.actual_execution_digest = hex(&right.starting_execution_digest.as_bytes());
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    if left.final_observable_digest != right.starting_observable_digest {
        evidence.expected_observable_digest = hex(&left.final_observable_digest.as_bytes());
        evidence.actual_observable_digest = hex(&right.starting_observable_digest.as_bytes());
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    if left.final_revision != right.starting_revision {
        evidence.expected_revision = Some(left.final_revision.value());
        evidence.actual_revision = Some(right.starting_revision.value());
        return Some(replay_fail(
            ReplayVariant::LogsNotConcatenable,
            EvidenceVariant::LogsNotConcatenable,
            evidence,
        ));
    }
    None
}

fn rematerialize<D>(
    machine: &Machine<D>,
    frame: &ReplayFrame<D>,
    evidence: &ReplayEvidence,
) -> Result<Transaction<D>, ReplayFailure<D>> {
    let compiled = machine.compiled();
    let at = frame.transaction.requested_time();
    let revision = frame.transaction.expected_revision();
    if let Some(snapshot) = frame.transaction.initialization_input() {
        if snapshot.network_fingerprint() != compiled.fingerprint() {
            let mut evidence = evidence.clone();
            evidence.expected_fingerprint = hex(&compiled.fingerprint().as_bytes());
            evidence.actual_fingerprint = hex(&snapshot.network_fingerprint().as_bytes());
            return Err(replay_fail(
                ReplayVariant::NetworkFingerprintMismatch,
                EvidenceVariant::NetworkFingerprintMismatch,
                evidence,
            ));
        }
        if snapshot.network_key() != compiled.network_key()
            || snapshot.input_schema_fingerprint() != compiled.input_schema_fingerprint()
        {
            return Ok(Transaction::initialize(at, revision, snapshot.clone()));
        }
        let mut builder = compiled.input_snapshot();
        for (key, level) in &snapshot.levels {
            builder = match builder.set(*key, *level) {
                Ok(builder) => builder,
                Err(failure) => return Err(materialization(failure.problem(), evidence)),
            };
        }
        for (key, count) in &snapshot.pulses {
            builder = match builder.pulse(*key, *count) {
                Ok(builder) => builder,
                Err(failure) => return Err(materialization(failure.problem(), evidence)),
            };
        }
        let input = match builder.finish() {
            Ok(input) => input,
            Err(failure) => return Err(materialization(failure.problem(), evidence)),
        };
        return Ok(Transaction::initialize(at, revision, input));
    }
    let Some(delta) = frame.transaction.advance_input() else {
        return Err(ReplayFailure::Decode(malformed(
            "replay_frame",
            "transaction",
            "missing input",
        )));
    };
    if delta.network_fingerprint() != compiled.fingerprint() {
        let mut evidence = evidence.clone();
        evidence.expected_fingerprint = hex(&compiled.fingerprint().as_bytes());
        evidence.actual_fingerprint = hex(&delta.network_fingerprint().as_bytes());
        return Err(replay_fail(
            ReplayVariant::NetworkFingerprintMismatch,
            EvidenceVariant::NetworkFingerprintMismatch,
            evidence,
        ));
    }
    if delta.network_key() != compiled.network_key()
        || delta.input_schema_fingerprint() != compiled.input_schema_fingerprint()
    {
        return Ok(Transaction::advance(at, revision, delta.clone()));
    }
    let mut builder = compiled.input_delta();
    for (key, level) in &delta.levels {
        builder = match builder.set(*key, *level) {
            Ok(builder) => builder,
            Err(failure) => return Err(materialization(failure.problem(), evidence)),
        };
    }
    for (key, count) in &delta.pulses {
        builder = match builder.pulse(*key, *count) {
            Ok(builder) => builder,
            Err(failure) => return Err(materialization(failure.problem(), evidence)),
        };
    }
    match builder.finish() {
        Ok(input) => Ok(Transaction::advance(at, revision, input)),
        Err(failure) => Err(materialization(failure.problem(), evidence)),
    }
}

fn materialization<D>(problem: Problem<D>, evidence: &ReplayEvidence) -> ReplayFailure<D> {
    ReplayFailure::Materialization {
        problem,
        frame_index: evidence.frame_index.unwrap_or(0),
        logical_time: evidence.logical_time,
    }
}

fn frame_evidence<D>(frame: &ReplayFrame<D>, log_id: &str) -> ReplayEvidence {
    let mut evidence = ReplayEvidence::new();
    evidence.log_id = log_id.to_owned();
    evidence.frame_index = Some(frame.frame_index);
    evidence.frame_id = frame.frame_index.to_string();
    evidence.logical_time = Some(frame.transaction.requested_time().ticks());
    evidence
}

struct Encoded {
    bytes: Vec<u8>,
    integrity: [u8; 32],
}

fn encoded_frame<D>(frame: &ReplayFrame<D>) -> Encoded {
    seal("replay_frame", frame.time_domain, &frame_payload(frame))
}

fn encoded_log<D>(log: &ReplayLog<D>) -> Encoded {
    seal("replay_log", log.time_domain, &log_payload(log))
}

fn frame_payload<D>(frame: &ReplayFrame<D>) -> Vec<u8> {
    let transaction = encoded_transaction(frame);
    let mut record = Record::new();
    record.field("expected_previous_execution_digest", |writer| {
        writer.bytes(&frame.expected_previous_execution_digest.as_bytes());
    });
    record.field("expected_revision", |writer| {
        writer.uint(frame.expected_revision.value());
    });
    record.field("frame_index", |writer| writer.uint(frame.frame_index));
    record.field("resulting_execution_digest", |writer| {
        writer.bytes(&frame.resulting_execution_digest.as_bytes());
    });
    record.field("resulting_observable_digest", |writer| {
        writer.bytes(&frame.resulting_observable_digest.as_bytes());
    });
    record.field("runtime_policy_id", |writer| {
        writer.bytes(&frame.runtime_policy_id.as_bytes());
    });
    record.field("transaction_record", |writer| {
        writer.bytes(&transaction.bytes);
    });
    record.finish()
}

fn encoded_transaction<D>(frame: &ReplayFrame<D>) -> Encoded {
    let input = encoded_input(&frame.transaction, frame.time_domain);
    let mut record = Record::new();
    record.field("expected_revision", |writer| {
        writer.uint(frame.transaction.expected_revision().value());
    });
    record.field("input", |writer| {
        writer.variant_start(input.kind);
        writer.bytes(&input.encoded.bytes);
    });
    record.field("input_schema_fingerprint", |writer| {
        writer.bytes(&input.schema.as_bytes());
    });
    record.field("network_fingerprint", |writer| {
        writer.bytes(&input.fingerprint.as_bytes());
    });
    record.field("network_key", |writer| {
        writer.bytes(&input.network_key.as_u128().to_be_bytes());
    });
    record.field("requested_time", |writer| {
        writer.uint(frame.transaction.requested_time().ticks());
    });
    seal("transaction_record", frame.time_domain, &record.finish())
}

struct EncodedInput {
    kind: &'static str,
    encoded: Encoded,
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    schema: InputSchemaFingerprint,
}

fn encoded_input<D>(transaction: &Transaction<D>, time_domain: TimeDomainId) -> EncodedInput {
    if let Some(snapshot) = transaction.initialization_input() {
        return EncodedInput {
            kind: "input_snapshot",
            encoded: seal(
                "input_snapshot",
                time_domain,
                &input_payload(
                    snapshot.network_key(),
                    snapshot.network_fingerprint(),
                    snapshot.input_schema_fingerprint(),
                    &snapshot.levels,
                    &snapshot.pulses,
                ),
            ),
            network_key: snapshot.network_key(),
            fingerprint: snapshot.network_fingerprint(),
            schema: snapshot.input_schema_fingerprint(),
        };
    }
    let Some(delta) = transaction.advance_input() else {
        panic!("a transaction is either initialization or advancement");
    };
    EncodedInput {
        kind: "input_delta",
        encoded: seal(
            "input_delta",
            time_domain,
            &input_payload(
                delta.network_key(),
                delta.network_fingerprint(),
                delta.input_schema_fingerprint(),
                &delta.levels,
                &delta.pulses,
            ),
        ),
        network_key: delta.network_key(),
        fingerprint: delta.network_fingerprint(),
        schema: delta.input_schema_fingerprint(),
    }
}

fn input_payload(
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    schema: InputSchemaFingerprint,
    levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
) -> Vec<u8> {
    let mut record = Record::new();
    record.field("input_schema_fingerprint", |writer| {
        writer.bytes(&schema.as_bytes());
    });
    record.field("levels", |writer| write_levels(writer, levels));
    record.field("network_fingerprint", |writer| {
        writer.bytes(&fingerprint.as_bytes());
    });
    record.field("network_key", |writer| {
        writer.bytes(&network_key.as_u128().to_be_bytes());
    });
    record.field("pulses", |writer| write_pulses(writer, pulses));
    record.finish()
}

fn write_levels(writer: &mut Cbor, levels: &BTreeMap<ExternalInputKey<Level>, LogicLevel>) {
    writer.array_start(levels.len());
    for (key, level) in levels {
        writer.array_start(2);
        writer.bytes(&key.as_u128().to_be_bytes());
        writer.variant_null(level_name(*level));
    }
}

fn write_pulses(writer: &mut Cbor, pulses: &BTreeMap<ExternalInputKey<Pulse>, PulseCount>) {
    let positive = pulses
        .iter()
        .filter(|(_, count)| count.is_positive())
        .count();
    writer.array_start(positive);
    for (key, count) in pulses {
        if !count.is_positive() {
            continue;
        }
        writer.array_start(2);
        writer.bytes(&key.as_u128().to_be_bytes());
        writer.uint(count.get());
    }
}

fn level_name(level: LogicLevel) -> &'static str {
    match level {
        LogicLevel::Low => "low",
        LogicLevel::High => "high",
    }
}

fn log_payload<D>(log: &ReplayLog<D>) -> Vec<u8> {
    let frames = log
        .frames
        .iter()
        .map(|frame| encoded_frame(frame).bytes)
        .collect::<Vec<_>>();
    let mut record = Record::new();
    record.field("final_execution_digest", |writer| {
        writer.bytes(&log.final_execution_digest.as_bytes());
    });
    record.field("final_observable_digest", |writer| {
        writer.bytes(&log.final_observable_digest.as_bytes());
    });
    record.field("final_revision", |writer| {
        writer.uint(log.final_revision.value())
    });
    record.field("frames", |writer| {
        // SPEC: docs/specs/contracts/replay-artifacts.yaml "concatenation"
        // Frame order is semantic and must not be sorted by content.
        writer.array_start(frames.len());
        for frame in &frames {
            writer.bytes(frame);
        }
    });
    record.field("network_fingerprint", |writer| {
        writer.bytes(&log.network_fingerprint.as_bytes());
    });
    record.field("network_key", |writer| {
        writer.bytes(&log.network_key.as_u128().to_be_bytes());
    });
    record.field("runtime_policy_id", |writer| {
        writer.bytes(&log.runtime_policy_id.as_bytes());
    });
    record.field("semantic_versions", |writer| {
        writer.nested(&version_record(log.versions));
    });
    record.field("starting_execution_digest", |writer| {
        writer.bytes(&log.starting_execution_digest.as_bytes());
    });
    record.field("starting_observable_digest", |writer| {
        writer.bytes(&log.starting_observable_digest.as_bytes());
    });
    record.field("starting_revision", |writer| {
        writer.uint(log.starting_revision.value());
    });
    record.field("time_domain_id", |writer| {
        writer.bytes(&log.time_domain.to_be_bytes());
    });
    record.finish()
}

fn version_record(versions: SemanticVersions) -> Vec<u8> {
    let mut record = Record::new();
    for name in VERSION_FIELDS {
        let version = versions.component(name).unwrap_or(VERSION);
        record.field(name, |writer| writer.uint(version));
    }
    record.finish()
}

fn log_content_digest(
    starting: &Checkpoint,
    ending: &Checkpoint,
    versions: &SemanticVersions,
    frame_digests: &[[u8; 32]],
) -> ReplayLogContentDigest {
    let mut digests = Cbor::default();
    digests.array_start(frame_digests.len());
    for digest in frame_digests {
        digests.bytes(digest);
    }
    let digest_bytes = digests.finish();
    let mut record = Record::new();
    record.field("final_execution_digest", |writer| {
        writer.bytes(&ending.execution.as_bytes());
    });
    record.field("final_observable_digest", |writer| {
        writer.bytes(&ending.observable.as_bytes());
    });
    record.field("final_revision", |writer| {
        writer.uint(ending.revision.value())
    });
    record.field("frame_digests", |writer| writer.nested(&digest_bytes));
    record.field("network_fingerprint", |writer| {
        writer.bytes(&starting.network_fingerprint.as_bytes());
    });
    record.field("network_key", |writer| {
        writer.bytes(&starting.network_key.as_u128().to_be_bytes());
    });
    record.field("runtime_policy_id", |writer| {
        writer.bytes(&starting.policy.as_bytes());
    });
    record.field("semantic_versions", |writer| {
        writer.nested(&version_record(*versions));
    });
    record.field("starting_execution_digest", |writer| {
        writer.bytes(&starting.execution.as_bytes());
    });
    record.field("starting_observable_digest", |writer| {
        writer.bytes(&starting.observable.as_bytes());
    });
    record.field("starting_revision", |writer| {
        writer.uint(starting.revision.value())
    });
    record.field("time_domain_id", |writer| {
        writer.bytes(&starting.time_domain.to_be_bytes());
    });
    // SPEC: docs/specs/contracts/replay-artifacts.yaml "log-content-digest"
    // Each embedded frame contributes its artifact integrity digest.
    let digest = blake3::hash(&domain_separated(
        REPLAY_LOG_CONTENT_DOMAIN,
        1,
        &record.finish(),
    ));
    ReplayLogContentDigest::from_digest(*digest.as_bytes())
}

fn seal(kind: &str, time_domain: TimeDomainId, payload: &[u8]) -> Encoded {
    let bare = envelope(kind, time_domain, payload, None);
    let integrity =
        *blake3::hash(&domain_separated(ARTIFACT_INTEGRITY_DOMAIN, 2, &bare)).as_bytes();
    let signed = envelope(kind, time_domain, payload, Some(integrity));
    Encoded {
        bytes: standalone(&signed),
        integrity,
    }
}

fn envelope(
    kind: &str,
    time_domain: TimeDomainId,
    payload: &[u8],
    integrity: Option<[u8; 32]>,
) -> Vec<u8> {
    envelope_at(kind, time_domain, payload, integrity, VERSION)
}

fn envelope_at(
    kind: &str,
    time_domain: TimeDomainId,
    payload: &[u8],
    integrity: Option<[u8; 32]>,
    version: u64,
) -> Vec<u8> {
    let mut record = Record::new();
    record.field("artifact_kind", |writer| writer.text(kind));
    for name in VERSION_FIELDS {
        record.field(name, |writer| writer.uint(version));
    }
    if let Some(digest) = integrity {
        record.field("integrity_digest", |writer| writer.bytes(&digest));
    }
    record.field("payload", |writer| writer.nested(payload));
    record.field("time_domain_id", |writer| {
        writer.bytes(&time_domain.to_be_bytes());
    });
    record.finish()
}

fn standalone(envelope: &[u8]) -> Vec<u8> {
    let mut body = Cbor::default();
    body.variant_start("mossignal_artifact");
    body.nested(envelope);
    let mut bytes = ARTIFACT_PREFIX.to_vec();
    bytes.extend(body.finish());
    bytes
}

struct Record {
    fields: Vec<(&'static str, Vec<u8>)>,
}

impl Record {
    fn new() -> Self {
        Self { fields: Vec::new() }
    }

    fn field(&mut self, name: &'static str, write: impl FnOnce(&mut Cbor)) {
        let mut writer = Cbor::default();
        write(&mut writer);
        self.fields.push((name, writer.finish()));
    }

    fn finish(mut self) -> Vec<u8> {
        self.fields.sort_by(|left, right| left.0.cmp(right.0));
        for pair in self.fields.windows(2) {
            if pair[0].0 == pair[1].0 {
                panic!("canonical record field names must be unique");
            }
        }
        let mut writer = Cbor::default();
        writer.record_start(self.fields.len());
        for (name, value) in &self.fields {
            writer.array_start(2);
            writer.text(name);
            writer.nested(value);
        }
        writer.finish()
    }
}

struct Opened {
    time_domain: TimeDomainId,
    payload: Value,
    integrity: [u8; 32],
    versions: SemanticVersions,
}

fn open_kind<D>(
    bytes: &[u8],
    policy: &DecodePolicy,
    expected: &'static str,
) -> Result<Opened, ReplayFailure<D>> {
    open_artifact(bytes, policy, expected).map_err(ReplayFailure::Decode)
}

fn open_artifact<D>(
    bytes: &[u8],
    policy: &DecodePolicy,
    expected: &str,
) -> Result<Opened, crate::DecodeFailure<D>> {
    if bytes.len() as u64 > policy.total_bytes() {
        return Err(limit_failure(
            expected,
            "total_bytes",
            policy.total_bytes(),
            bytes.len() as u64,
        ));
    }
    if bytes.len() < ARTIFACT_PREFIX.len() {
        return Err(if ARTIFACT_PREFIX.starts_with(bytes) {
            canonical(
                expected,
                CanonicalFault::Truncated,
                "prefix",
                "truncated_artifact",
                bytes.len().to_string(),
            )
        } else {
            canonical(
                expected,
                CanonicalFault::Prefix,
                "prefix",
                "invalid_prefix",
                bytes.len().to_string(),
            )
        });
    }
    if bytes[..ARTIFACT_PREFIX.len()] != ARTIFACT_PREFIX {
        return Err(canonical(
            expected,
            CanonicalFault::Prefix,
            "prefix",
            "invalid_prefix",
            hex(&bytes[..ARTIFACT_PREFIX.len()]),
        ));
    }
    if bytes.len() == ARTIFACT_PREFIX.len() {
        return Err(canonical(
            expected,
            CanonicalFault::Truncated,
            "body",
            "truncated_artifact",
            "prefix only",
        ));
    }
    let body = &bytes[ARTIFACT_PREFIX.len()..];
    let (value, consumed) =
        cbor_decode::parse(body, cbor_limits(policy)).map_err(|error| map_cbor(expected, error))?;
    if consumed != body.len() {
        return Err(canonical(
            expected,
            CanonicalFault::Trailing,
            "body",
            "trailing_bytes",
            (body.len() - consumed).to_string(),
        ));
    }
    if cbor_decode::encode(&value) != body {
        return Err(canonical(
            expected,
            CanonicalFault::Noncanonical,
            "body",
            "re_encode_mismatch",
            "body",
        ));
    }
    let mut items = expect_array(expected, value, "artifact")?;
    if items.len() != 2 {
        return Err(malformed(expected, "artifact", "wrapper length"));
    }
    let name = expect_text(expected, items.remove(0), "artifact.kind")?;
    if name != "mossignal_artifact" {
        return Err(malformed(expected, "artifact", name));
    }
    let mut fields = into_fields(expected, items.remove(0), "envelope")?;
    reject_unknown(expected, &fields, ENVELOPE_FIELDS, "envelope")?;
    // SPEC: docs/specs/contracts/replay-artifacts.yaml "hostile-and-version-gate"
    // Kind is identified before integrity. The digest still covers artifact_kind.
    let kind = match fields.get("artifact_kind") {
        Some(Value::Text(kind)) => kind.clone(),
        Some(_) => return Err(malformed(expected, "envelope.artifact_kind", "text")),
        None => return Err(malformed(expected, "envelope", "missing artifact_kind")),
    };
    if kind != expected {
        return Err(if recognized_kind(&kind) {
            malformed(expected, "envelope.artifact_kind", kind)
        } else {
            unknown_kind(expected, &kind)
        });
    }
    let integrity = expect_digest(
        expected,
        take_required(expected, &mut fields, "integrity_digest", "envelope")?,
        "envelope.integrity_digest",
    )?;
    let pairs = fields
        .iter()
        .map(|(name, value)| (name.as_str(), value))
        .collect::<Vec<_>>();
    let bare = cbor_decode::encode_named_pairs(&pairs);
    let actual = *blake3::hash(&domain_separated(ARTIFACT_INTEGRITY_DOMAIN, 2, &bare)).as_bytes();
    if actual != integrity {
        return Err(crate::DecodeFailure::IntegrityDigestMismatch(
            persistence_problem(ProblemEvidence::PersistenceIntegrityDigestMismatch {
                evidence: DigestMismatchEvidence {
                    kind: "artifact_integrity",
                    expected: hex(&integrity),
                    actual: hex(&actual),
                    context: expected.to_owned(),
                },
                marker: PhantomData,
            }),
        ));
    }
    let mut versions = SemanticVersions::CURRENT;
    for name in VERSION_FIELDS {
        let version = expect_uint(
            expected,
            fields
                .get(*name)
                .ok_or_else(|| malformed(expected, "envelope", *name))?,
            &format!("envelope.{name}"),
        )?;
        if version != VERSION {
            return Err(unsupported_version(
                expected,
                "envelope",
                name,
                version,
                VERSION.to_string(),
            ));
        }
        set_version(&mut versions, name, version);
    }
    let time_domain = TimeDomainId::from_u128(expect_key(
        expected,
        take_required(expected, &mut fields, "time_domain_id", "envelope")?,
        "envelope.time_domain_id",
    )?);
    let payload = take_required(expected, &mut fields, "payload", "envelope")?;
    Ok(Opened {
        time_domain,
        payload,
        integrity,
        versions,
    })
}

fn set_version(versions: &mut SemanticVersions, name: &str, version: u64) {
    match name {
        "artifact_schema_version" => versions.artifact_schema_version = version,
        "canonical_encoding_version" => versions.canonical_encoding_version = version,
        "core_semantics_version" => versions.core_semantics_version = version,
        "diagnostic_schema_version" => versions.diagnostic_schema_version = version,
        "digest_suite_version" => versions.digest_suite_version = version,
        "envelope_schema_version" => versions.envelope_schema_version = version,
        "node_semantics_version" => versions.node_semantics_version = version,
        "patch_semantics_version" => versions.patch_semantics_version = version,
        "provenance_semantics_version" => versions.provenance_semantics_version = version,
        _ => {}
    }
}

fn parse_log<D>(opened: Opened, policy: &DecodePolicy) -> Result<ReplayLog<D>, ReplayFailure<D>> {
    let mut fields =
        into_fields("replay_log", opened.payload, "payload").map_err(ReplayFailure::Decode)?;
    reject_unknown("replay_log", &fields, LOG_FIELDS, "payload").map_err(ReplayFailure::Decode)?;
    let versions = parse_versions("replay_log", &mut fields, &opened.versions)?;
    let time_domain = TimeDomainId::from_u128(
        expect_key(
            "replay_log",
            take_required("replay_log", &mut fields, "time_domain_id", "payload")?,
            "payload.time_domain_id",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    if time_domain != opened.time_domain {
        return Err(ReplayFailure::Decode(wrong_time_domain(
            "replay_log",
            "payload",
            &hex(&opened.time_domain.to_be_bytes()),
            &hex(&time_domain.to_be_bytes()),
        )));
    }
    let frames_value = take_required("replay_log", &mut fields, "frames", "payload")
        .map_err(ReplayFailure::Decode)?;
    let frame_bytes = expect_array("replay_log", frames_value, "payload.frames")
        .map_err(ReplayFailure::Decode)?;
    let count = u64::try_from(frame_bytes.len()).unwrap_or(u64::MAX);
    if count > policy.replay_frames() {
        return Err(ReplayFailure::Decode(limit_failure(
            "replay_log",
            "replay_frames",
            policy.replay_frames(),
            count,
        )));
    }
    let mut frames = Vec::with_capacity(frame_bytes.len());
    let mut integrity = Vec::with_capacity(frame_bytes.len());
    for value in frame_bytes {
        let bytes =
            expect_bytes("replay_log", value, "payload.frames").map_err(ReplayFailure::Decode)?;
        let child = open_artifact(&bytes, policy, "replay_frame").map_err(ReplayFailure::Decode)?;
        if child.time_domain != opened.time_domain {
            return Err(ReplayFailure::Decode(wrong_time_domain(
                "replay_frame",
                "log",
                &hex(&opened.time_domain.to_be_bytes()),
                &hex(&child.time_domain.to_be_bytes()),
            )));
        }
        let (frame, frame_integrity) = parse_frame(child, policy)?;
        integrity.push(frame_integrity);
        frames.push(frame);
    }
    let network_key = NetworkKey::from_u128(
        expect_key(
            "replay_log",
            take_required("replay_log", &mut fields, "network_key", "payload")?,
            "payload.network_key",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let network_fingerprint = NetworkFingerprint::from_digest(
        expect_digest(
            "replay_log",
            take_required("replay_log", &mut fields, "network_fingerprint", "payload")?,
            "payload.network_fingerprint",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let policy_id = RuntimePolicyId::from_digest(
        expect_digest(
            "replay_log",
            take_required("replay_log", &mut fields, "runtime_policy_id", "payload")?,
            "payload.runtime_policy_id",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let starting_revision = NetworkRevision::from_value(
        expect_uint(
            "replay_log",
            &take_required("replay_log", &mut fields, "starting_revision", "payload")?,
            "payload.starting_revision",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let starting_execution = ExecutionStateDigest::from_digest(
        expect_digest(
            "replay_log",
            take_required(
                "replay_log",
                &mut fields,
                "starting_execution_digest",
                "payload",
            )?,
            "payload.starting_execution_digest",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let starting_observable = ObservableStateDigest::from_digest(
        expect_digest(
            "replay_log",
            take_required(
                "replay_log",
                &mut fields,
                "starting_observable_digest",
                "payload",
            )?,
            "payload.starting_observable_digest",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let final_revision = NetworkRevision::from_value(
        expect_uint(
            "replay_log",
            &take_required("replay_log", &mut fields, "final_revision", "payload")?,
            "payload.final_revision",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let final_execution = ExecutionStateDigest::from_digest(
        expect_digest(
            "replay_log",
            take_required(
                "replay_log",
                &mut fields,
                "final_execution_digest",
                "payload",
            )?,
            "payload.final_execution_digest",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let final_observable = ObservableStateDigest::from_digest(
        expect_digest(
            "replay_log",
            take_required(
                "replay_log",
                &mut fields,
                "final_observable_digest",
                "payload",
            )?,
            "payload.final_observable_digest",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let _ = opened.integrity;
    Ok(ReplayLog {
        time_domain,
        network_key,
        network_fingerprint,
        starting_revision,
        starting_execution_digest: starting_execution,
        starting_observable_digest: starting_observable,
        runtime_policy_id: policy_id,
        versions,
        frames,
        final_execution_digest: final_execution,
        final_observable_digest: final_observable,
        final_revision,
        content_digest: log_content_digest(
            &Checkpoint {
                time_domain,
                network_key,
                network_fingerprint,
                revision: starting_revision,
                execution: starting_execution,
                observable: starting_observable,
                policy: policy_id,
            },
            &Checkpoint {
                time_domain,
                network_key,
                network_fingerprint,
                revision: final_revision,
                execution: final_execution,
                observable: final_observable,
                policy: policy_id,
            },
            &versions,
            &integrity,
        ),
    })
}

fn parse_frame<D>(
    opened: Opened,
    policy: &DecodePolicy,
) -> Result<(ReplayFrame<D>, [u8; 32]), ReplayFailure<D>> {
    let mut fields =
        into_fields("replay_frame", opened.payload, "payload").map_err(ReplayFailure::Decode)?;
    reject_unknown("replay_frame", &fields, FRAME_FIELDS, "payload")
        .map_err(ReplayFailure::Decode)?;
    if let Some(result) = fields.remove("transaction_result") {
        return Err(reject_transaction_result(result, policy));
    }
    let transaction_bytes = expect_bytes(
        "replay_frame",
        take_required("replay_frame", &mut fields, "transaction_record", "payload")?,
        "payload.transaction_record",
    )
    .map_err(ReplayFailure::Decode)?;
    let transaction = parse_transaction(&transaction_bytes, policy, opened.time_domain)?;
    let frame = ReplayFrame {
        frame_index: expect_uint(
            "replay_frame",
            &take_required("replay_frame", &mut fields, "frame_index", "payload")?,
            "payload.frame_index",
        )
        .map_err(ReplayFailure::Decode)?,
        time_domain: opened.time_domain,
        expected_previous_execution_digest: ExecutionStateDigest::from_digest(
            expect_digest(
                "replay_frame",
                take_required(
                    "replay_frame",
                    &mut fields,
                    "expected_previous_execution_digest",
                    "payload",
                )?,
                "payload.expected_previous_execution_digest",
            )
            .map_err(ReplayFailure::Decode)?,
        ),
        expected_revision: NetworkRevision::from_value(
            expect_uint(
                "replay_frame",
                &take_required("replay_frame", &mut fields, "expected_revision", "payload")?,
                "payload.expected_revision",
            )
            .map_err(ReplayFailure::Decode)?,
        ),
        runtime_policy_id: RuntimePolicyId::from_digest(
            expect_digest(
                "replay_frame",
                take_required("replay_frame", &mut fields, "runtime_policy_id", "payload")?,
                "payload.runtime_policy_id",
            )
            .map_err(ReplayFailure::Decode)?,
        ),
        transaction,
        resulting_execution_digest: ExecutionStateDigest::from_digest(
            expect_digest(
                "replay_frame",
                take_required(
                    "replay_frame",
                    &mut fields,
                    "resulting_execution_digest",
                    "payload",
                )?,
                "payload.resulting_execution_digest",
            )
            .map_err(ReplayFailure::Decode)?,
        ),
        resulting_observable_digest: ObservableStateDigest::from_digest(
            expect_digest(
                "replay_frame",
                take_required(
                    "replay_frame",
                    &mut fields,
                    "resulting_observable_digest",
                    "payload",
                )?,
                "payload.resulting_observable_digest",
            )
            .map_err(ReplayFailure::Decode)?,
        ),
    };
    Ok((frame, opened.integrity))
}

fn reject_transaction_result<D>(value: Value, policy: &DecodePolicy) -> ReplayFailure<D> {
    let bytes = match expect_bytes("replay_frame", value, "payload.transaction_result") {
        Ok(bytes) => bytes,
        Err(failure) => return ReplayFailure::Decode(failure),
    };
    let opened = match open_artifact(&bytes, policy, "transaction_result") {
        Ok(opened) => opened,
        Err(failure) => return ReplayFailure::Decode(failure),
    };
    ReplayFailure::Decode(unsupported_version(
        "transaction_result",
        "payload",
        "artifact_schema_version",
        opened.versions.artifact_schema_version,
        "none".to_owned(),
    ))
}

fn parse_transaction<D>(
    bytes: &[u8],
    policy: &DecodePolicy,
    time_domain: TimeDomainId,
) -> Result<Transaction<D>, ReplayFailure<D>> {
    let opened =
        open_artifact(bytes, policy, "transaction_record").map_err(ReplayFailure::Decode)?;
    if opened.time_domain != time_domain {
        return Err(ReplayFailure::Decode(wrong_time_domain(
            "transaction_record",
            "frame",
            &hex(&time_domain.to_be_bytes()),
            &hex(&opened.time_domain.to_be_bytes()),
        )));
    }
    let mut fields = into_fields("transaction_record", opened.payload, "payload")
        .map_err(ReplayFailure::Decode)?;
    if fields.contains_key("patch") {
        let mut evidence = ReplayEvidence::new();
        evidence.underlying_code = "patch".to_owned();
        return Err(replay_fail(
            ReplayVariant::PatchPreparationDiverged,
            EvidenceVariant::PatchPreparationDiverged,
            evidence,
        ));
    }
    reject_unknown("transaction_record", &fields, TRANSACTION_FIELDS, "payload")
        .map_err(ReplayFailure::Decode)?;
    let requested = expect_uint(
        "transaction_record",
        &take_required(
            "transaction_record",
            &mut fields,
            "requested_time",
            "payload",
        )?,
        "payload.requested_time",
    )
    .map_err(ReplayFailure::Decode)?;
    let revision = NetworkRevision::from_value(
        expect_uint(
            "transaction_record",
            &take_required(
                "transaction_record",
                &mut fields,
                "expected_revision",
                "payload",
            )?,
            "payload.expected_revision",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let network_key = NetworkKey::from_u128(
        expect_key(
            "transaction_record",
            take_required("transaction_record", &mut fields, "network_key", "payload")?,
            "payload.network_key",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let fingerprint = NetworkFingerprint::from_digest(
        expect_digest(
            "transaction_record",
            take_required(
                "transaction_record",
                &mut fields,
                "network_fingerprint",
                "payload",
            )?,
            "payload.network_fingerprint",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let schema = InputSchemaFingerprint::from_digest(
        expect_digest(
            "transaction_record",
            take_required(
                "transaction_record",
                &mut fields,
                "input_schema_fingerprint",
                "payload",
            )?,
            "payload.input_schema_fingerprint",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let input = take_required("transaction_record", &mut fields, "input", "payload")
        .map_err(ReplayFailure::Decode)?;
    let (kind, body) = expect_variant("transaction_record", input, "payload.input")
        .map_err(ReplayFailure::Decode)?;
    if kind != "input_snapshot" && kind != "input_delta" {
        return Err(ReplayFailure::Decode(unknown_variant(
            "transaction_record",
            "payload.input",
            &kind,
        )));
    }
    let child =
        expect_bytes("transaction_record", body, "payload.input").map_err(ReplayFailure::Decode)?;
    let observations = parse_input(
        &child,
        policy,
        kind.as_str(),
        time_domain,
        network_key,
        fingerprint,
        schema,
    )?;
    let at = crate::time::Time::from_ticks(requested);
    if kind == "input_snapshot" {
        Ok(Transaction::initialize(at, revision, observations.snapshot))
    } else {
        Ok(Transaction::advance(at, revision, observations.delta))
    }
}

struct Observations<D> {
    snapshot: InputSnapshot<D>,
    delta: InputDelta<D>,
}

fn parse_input<D>(
    bytes: &[u8],
    policy: &DecodePolicy,
    kind: &str,
    time_domain: TimeDomainId,
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    schema: InputSchemaFingerprint,
) -> Result<Observations<D>, ReplayFailure<D>> {
    let opened = open_artifact(bytes, policy, kind).map_err(ReplayFailure::Decode)?;
    if opened.time_domain != time_domain {
        return Err(ReplayFailure::Decode(wrong_time_domain(
            kind,
            "transaction",
            &hex(&time_domain.to_be_bytes()),
            &hex(&opened.time_domain.to_be_bytes()),
        )));
    }
    let mut fields = into_fields(kind, opened.payload, "payload").map_err(ReplayFailure::Decode)?;
    reject_unknown(kind, &fields, INPUT_FIELDS, "payload").map_err(ReplayFailure::Decode)?;
    let child_key = NetworkKey::from_u128(
        expect_key(
            kind,
            take_required(kind, &mut fields, "network_key", "payload")?,
            "payload.network_key",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let child_fingerprint = NetworkFingerprint::from_digest(
        expect_digest(
            kind,
            take_required(kind, &mut fields, "network_fingerprint", "payload")?,
            "payload.network_fingerprint",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    let child_schema = InputSchemaFingerprint::from_digest(
        expect_digest(
            kind,
            take_required(kind, &mut fields, "input_schema_fingerprint", "payload")?,
            "payload.input_schema_fingerprint",
        )
        .map_err(ReplayFailure::Decode)?,
    );
    if child_key != network_key || child_fingerprint != fingerprint || child_schema != schema {
        return Err(ReplayFailure::Decode(malformed(kind, "payload", "binding")));
    }
    let levels = parse_levels(
        kind,
        take_required(kind, &mut fields, "levels", "payload").map_err(ReplayFailure::Decode)?,
    )?;
    let pulses = parse_pulses(
        kind,
        take_required(kind, &mut fields, "pulses", "payload").map_err(ReplayFailure::Decode)?,
    )?;
    Ok(Observations {
        snapshot: InputSnapshot::from_decoded(
            network_key,
            fingerprint,
            schema,
            levels.clone(),
            pulses.clone(),
        ),
        delta: InputDelta::from_decoded(network_key, fingerprint, schema, levels, pulses),
    })
}

fn parse_levels<D>(
    kind: &str,
    value: Value,
) -> Result<BTreeMap<ExternalInputKey<Level>, LogicLevel>, ReplayFailure<D>> {
    let items = expect_array(kind, value, "payload.levels").map_err(ReplayFailure::Decode)?;
    let mut levels = BTreeMap::new();
    let mut previous: Option<u128> = None;
    for item in items {
        let mut pair = expect_array(kind, item, "payload.levels").map_err(ReplayFailure::Decode)?;
        if pair.len() != 2 {
            return Err(ReplayFailure::Decode(malformed(
                kind,
                "payload.levels",
                "pair",
            )));
        }
        let key = expect_key(kind, pair.remove(0), "payload.levels.key")
            .map_err(ReplayFailure::Decode)?;
        if previous.is_some_and(|previous| key <= previous) {
            return Err(ReplayFailure::Decode(canonical(
                kind,
                CanonicalFault::Noncanonical,
                "payload.levels",
                "unsorted_key",
                key.to_string(),
            )));
        }
        previous = Some(key);
        let (name, body) = expect_variant(kind, pair.remove(0), "payload.levels.level")
            .map_err(ReplayFailure::Decode)?;
        if !matches!(body, Value::Null) {
            return Err(ReplayFailure::Decode(malformed(
                kind,
                "payload.levels.level",
                "variant body",
            )));
        }
        let level = match name.as_str() {
            "low" => LogicLevel::Low,
            "high" => LogicLevel::High,
            _ => {
                return Err(ReplayFailure::Decode(unknown_variant(
                    kind,
                    "payload.levels.level",
                    &name,
                )));
            }
        };
        levels.insert(ExternalInputKey::from_u128(key), level);
    }
    Ok(levels)
}

fn parse_pulses<D>(
    kind: &str,
    value: Value,
) -> Result<BTreeMap<ExternalInputKey<Pulse>, PulseCount>, ReplayFailure<D>> {
    let items = expect_array(kind, value, "payload.pulses").map_err(ReplayFailure::Decode)?;
    let mut pulses = BTreeMap::new();
    let mut previous: Option<u128> = None;
    for item in items {
        let mut pair = expect_array(kind, item, "payload.pulses").map_err(ReplayFailure::Decode)?;
        if pair.len() != 2 {
            return Err(ReplayFailure::Decode(malformed(
                kind,
                "payload.pulses",
                "pair",
            )));
        }
        let key = expect_key(kind, pair.remove(0), "payload.pulses.key")
            .map_err(ReplayFailure::Decode)?;
        if previous.is_some_and(|previous| key <= previous) {
            return Err(ReplayFailure::Decode(canonical(
                kind,
                CanonicalFault::Noncanonical,
                "payload.pulses",
                "unsorted_key",
                key.to_string(),
            )));
        }
        previous = Some(key);
        let count = expect_uint(kind, &pair.remove(0), "payload.pulses.count")
            .map_err(ReplayFailure::Decode)?;
        if count == 0 {
            return Err(ReplayFailure::Decode(malformed(
                kind,
                "payload.pulses",
                "non_positive",
            )));
        }
        pulses.insert(ExternalInputKey::from_u128(key), PulseCount::new(count));
    }
    Ok(pulses)
}

fn parse_versions<D>(
    kind: &'static str,
    fields: &mut BTreeMap<String, Value>,
    envelope: &SemanticVersions,
) -> Result<SemanticVersions, ReplayFailure<D>> {
    let versions = into_fields(
        kind,
        take_required(kind, fields, "semantic_versions", "payload")
            .map_err(ReplayFailure::Decode)?,
        "payload.semantic_versions",
    )
    .map_err(ReplayFailure::Decode)?;
    reject_unknown(kind, &versions, VERSION_FIELDS, "payload.semantic_versions")
        .map_err(ReplayFailure::Decode)?;
    let mut parsed = SemanticVersions::CURRENT;
    for name in VERSION_FIELDS {
        let version = expect_uint(
            kind,
            versions
                .get(*name)
                .ok_or_else(|| malformed(kind, "payload.semantic_versions", *name))?,
            &format!("payload.semantic_versions.{name}"),
        )
        .map_err(ReplayFailure::Decode)?;
        let required = envelope.component(name).unwrap_or(VERSION);
        if version != required {
            return Err(ReplayFailure::Decode(unsupported_version(
                kind,
                "payload",
                name,
                version,
                required.to_string(),
            )));
        }
        set_version(&mut parsed, name, version);
    }
    Ok(parsed)
}

fn cbor_limits(policy: &DecodePolicy) -> Limits {
    Limits {
        total_bytes: policy.total_bytes(),
        nesting: policy.nesting(),
        text_bytes: policy.text_bytes(),
        byte_string_bytes: policy.byte_string_bytes(),
        collection_items: policy.collection_items(),
    }
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

enum CanonicalFault {
    Prefix,
    Truncated,
    Trailing,
    Noncanonical,
    Malformed,
}

fn persistence_problem<D>(evidence: ProblemEvidence<D>) -> Problem<D> {
    Problem::new(
        SubjectRef::Operation(OperationSubjectRef::Persistence),
        Vec::new(),
        evidence,
    )
}

fn replay_problem<D>(evidence: ProblemEvidence<D>) -> Problem<D> {
    Problem::new(
        SubjectRef::Operation(OperationSubjectRef::Persistence),
        Vec::new(),
        evidence,
    )
}

fn replay_fail<D>(
    variant: ReplayVariant,
    evidence_variant: EvidenceVariant,
    evidence: ReplayEvidence,
) -> ReplayFailure<D> {
    let problem = replay_problem(evidence_of(evidence_variant, evidence));
    match variant {
        ReplayVariant::StartingExecutionDigestMismatch => {
            ReplayFailure::StartingExecutionDigestMismatch(problem)
        }
        ReplayVariant::StartingObservableDigestMismatch => {
            ReplayFailure::StartingObservableDigestMismatch(problem)
        }
        ReplayVariant::ExpectedRevisionMismatch => ReplayFailure::ExpectedRevisionMismatch(problem),
        ReplayVariant::RuntimePolicyMismatch => ReplayFailure::RuntimePolicyMismatch(problem),
        ReplayVariant::TimeDomainMismatch => ReplayFailure::TimeDomainMismatch(problem),
        ReplayVariant::NetworkFingerprintMismatch => {
            ReplayFailure::NetworkFingerprintMismatch(problem)
        }
        ReplayVariant::LogsNotConcatenable => ReplayFailure::LogsNotConcatenable(problem),
        ReplayVariant::PatchPreparationDiverged => ReplayFailure::PatchPreparationDiverged(problem),
        ReplayVariant::ResultingExecutionDigestMismatch => {
            ReplayFailure::ResultingExecutionDigestMismatch(problem)
        }
        ReplayVariant::ResultingObservableDigestMismatch => {
            ReplayFailure::ResultingObservableDigestMismatch(problem)
        }
        ReplayVariant::FrameMissing => ReplayFailure::FrameMissing(problem),
        ReplayVariant::FrameReordered => ReplayFailure::FrameReordered(problem),
        ReplayVariant::FrameDuplicated => ReplayFailure::FrameDuplicated(problem),
    }
}

enum ReplayVariant {
    StartingExecutionDigestMismatch,
    StartingObservableDigestMismatch,
    ExpectedRevisionMismatch,
    RuntimePolicyMismatch,
    TimeDomainMismatch,
    NetworkFingerprintMismatch,
    LogsNotConcatenable,
    PatchPreparationDiverged,
    ResultingExecutionDigestMismatch,
    ResultingObservableDigestMismatch,
    FrameMissing,
    FrameReordered,
    FrameDuplicated,
}

enum EvidenceVariant {
    StartingExecutionDigestMismatch,
    StartingObservableDigestMismatch,
    ExpectedRevisionMismatch,
    RuntimePolicyMismatch,
    TimeDomainMismatch,
    NetworkFingerprintMismatch,
    LogsNotConcatenable,
    PatchPreparationDiverged,
    ResultingExecutionDigestMismatch,
    ResultingObservableDigestMismatch,
    FrameMissing,
    FrameReordered,
    FrameDuplicated,
}

fn evidence_of<D>(variant: EvidenceVariant, evidence: ReplayEvidence) -> ProblemEvidence<D> {
    match variant {
        EvidenceVariant::StartingExecutionDigestMismatch => {
            ProblemEvidence::ReplayStartingExecutionDigestMismatch {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::StartingObservableDigestMismatch => {
            ProblemEvidence::ReplayStartingObservableDigestMismatch {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::ExpectedRevisionMismatch => {
            ProblemEvidence::ReplayExpectedRevisionMismatch {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::RuntimePolicyMismatch => ProblemEvidence::ReplayRuntimePolicyMismatch {
            evidence,
            marker: PhantomData,
        },
        EvidenceVariant::TimeDomainMismatch => ProblemEvidence::ReplayTimeDomainMismatch {
            evidence,
            marker: PhantomData,
        },
        EvidenceVariant::NetworkFingerprintMismatch => {
            ProblemEvidence::ReplayNetworkFingerprintMismatch {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::LogsNotConcatenable => ProblemEvidence::ReplayLogsNotConcatenable {
            evidence,
            marker: PhantomData,
        },
        EvidenceVariant::PatchPreparationDiverged => {
            ProblemEvidence::ReplayPatchPreparationDiverged {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::ResultingExecutionDigestMismatch => {
            ProblemEvidence::ReplayResultingExecutionDigestMismatch {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::ResultingObservableDigestMismatch => {
            ProblemEvidence::ReplayResultingObservableDigestMismatch {
                evidence,
                marker: PhantomData,
            }
        }
        EvidenceVariant::FrameMissing => ProblemEvidence::ReplayFrameMissing {
            evidence,
            marker: PhantomData,
        },
        EvidenceVariant::FrameReordered => ProblemEvidence::ReplayFrameReordered {
            evidence,
            marker: PhantomData,
        },
        EvidenceVariant::FrameDuplicated => ProblemEvidence::ReplayFrameDuplicated {
            evidence,
            marker: PhantomData,
        },
    }
}

fn replay_evidence_of<D>(problem: &Problem<D>) -> Option<&ReplayEvidence> {
    match problem.evidence() {
        ProblemEvidence::ReplayStartingExecutionDigestMismatch { evidence, .. }
        | ProblemEvidence::ReplayStartingObservableDigestMismatch { evidence, .. }
        | ProblemEvidence::ReplayExpectedRevisionMismatch { evidence, .. }
        | ProblemEvidence::ReplayRuntimePolicyMismatch { evidence, .. }
        | ProblemEvidence::ReplayTimeDomainMismatch { evidence, .. }
        | ProblemEvidence::ReplayNetworkFingerprintMismatch { evidence, .. }
        | ProblemEvidence::ReplayLogsNotConcatenable { evidence, .. }
        | ProblemEvidence::ReplayPatchPreparationDiverged { evidence, .. }
        | ProblemEvidence::ReplayResultingExecutionDigestMismatch { evidence, .. }
        | ProblemEvidence::ReplayResultingObservableDigestMismatch { evidence, .. }
        | ProblemEvidence::ReplayFrameMissing { evidence, .. }
        | ProblemEvidence::ReplayFrameReordered { evidence, .. }
        | ProblemEvidence::ReplayFrameDuplicated { evidence, .. } => Some(evidence),
        _ => None,
    }
}

fn canonical<D>(
    kind: &str,
    fault: CanonicalFault,
    path: &str,
    violation: &'static str,
    encountered: impl Into<String>,
) -> crate::DecodeFailure<D> {
    let evidence = CanonicalEncodingEvidence {
        artifact_kind: Some(kind.to_owned()),
        path: path.to_owned(),
        violation,
        encountered: encountered.into(),
    };
    let problem = persistence_problem(match fault {
        CanonicalFault::Prefix => ProblemEvidence::PersistenceInvalidPrefix {
            evidence,
            marker: PhantomData,
        },
        CanonicalFault::Truncated => ProblemEvidence::PersistenceTruncatedArtifact {
            evidence,
            marker: PhantomData,
        },
        CanonicalFault::Trailing => ProblemEvidence::PersistenceTrailingBytes {
            evidence,
            marker: PhantomData,
        },
        CanonicalFault::Noncanonical => ProblemEvidence::PersistenceNoncanonicalEncoding {
            evidence,
            marker: PhantomData,
        },
        CanonicalFault::Malformed => ProblemEvidence::PersistenceMalformedEnvelope {
            evidence,
            marker: PhantomData,
        },
    });
    match fault {
        CanonicalFault::Prefix => crate::DecodeFailure::InvalidPrefix(problem),
        CanonicalFault::Truncated => crate::DecodeFailure::TruncatedArtifact(problem),
        CanonicalFault::Trailing => crate::DecodeFailure::TrailingBytes(problem),
        CanonicalFault::Noncanonical => crate::DecodeFailure::NoncanonicalEncoding(problem),
        CanonicalFault::Malformed => crate::DecodeFailure::MalformedEnvelope(problem),
    }
}

fn malformed<D>(kind: &str, path: &str, encountered: impl Into<String>) -> crate::DecodeFailure<D> {
    canonical(
        kind,
        CanonicalFault::Malformed,
        path,
        "malformed_envelope",
        encountered,
    )
}

fn limit_failure<D>(
    kind: &str,
    budget: &'static str,
    limit: u64,
    consumed: u64,
) -> crate::DecodeFailure<D> {
    let _ = kind;
    crate::DecodeFailure::DecodeLimitExceeded(persistence_problem(
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
    kind: &str,
    stage: &'static str,
    component: &str,
    encountered: u64,
    required: String,
) -> crate::DecodeFailure<D> {
    crate::DecodeFailure::UnsupportedVersion(persistence_problem(
        ProblemEvidence::PersistenceUnsupportedVersion {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: kind.to_owned(),
                stage,
                component: component.to_owned(),
                encountered: encountered.to_string(),
                required,
                upgrader_exists: false,
            },
            marker: PhantomData,
        },
    ))
}

fn unknown_kind<D>(expected: &str, kind: &str) -> crate::DecodeFailure<D> {
    crate::DecodeFailure::UnknownArtifactKind(persistence_problem(
        ProblemEvidence::PersistenceUnknownArtifactKind {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: kind.to_owned(),
                stage: "envelope",
                component: "artifact_kind".to_owned(),
                encountered: kind.to_owned(),
                required: expected.to_owned(),
                upgrader_exists: false,
            },
            marker: PhantomData,
        },
    ))
}

fn unknown_field<D>(kind: &str, path: &str, name: &str) -> crate::DecodeFailure<D> {
    crate::DecodeFailure::UnknownSchemaField(persistence_problem(
        ProblemEvidence::PersistenceUnknownSchemaField {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: kind.to_owned(),
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

fn unknown_variant<D>(kind: &str, path: &str, name: &str) -> crate::DecodeFailure<D> {
    crate::DecodeFailure::UnknownSchemaVariant(persistence_problem(
        ProblemEvidence::PersistenceUnknownSchemaVariant {
            evidence: VersionCompatibilityEvidence {
                artifact_kind: kind.to_owned(),
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

fn wrong_time_domain<D>(
    kind: &str,
    stage: &'static str,
    expected: &str,
    actual: &str,
) -> crate::DecodeFailure<D> {
    crate::DecodeFailure::WrongTimeDomain(persistence_problem(
        ProblemEvidence::PersistenceWrongTimeDomain {
            evidence: crate::diagnostics::ArtifactIdentityEvidence {
                artifact_kind: kind.to_owned(),
                stage,
                expected: expected.to_owned(),
                actual: actual.to_owned(),
            },
            marker: PhantomData,
        },
    ))
}

fn map_cbor<D>(kind: &str, error: DecodeError) -> crate::DecodeFailure<D> {
    match error {
        DecodeError::Truncated => canonical(
            kind,
            CanonicalFault::Truncated,
            "body",
            "truncated_artifact",
            "incomplete CBOR value",
        ),
        DecodeError::Noncanonical {
            violation,
            encountered,
        } => canonical(
            kind,
            CanonicalFault::Noncanonical,
            "body",
            violation,
            encountered,
        ),
        DecodeError::Limit {
            budget,
            limit,
            consumed,
        } => limit_failure(kind, budget, limit, consumed),
    }
}

fn expect_array<D>(
    kind: &str,
    value: Value,
    path: &str,
) -> Result<Vec<Value>, crate::DecodeFailure<D>> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(malformed(kind, path, "array")),
    }
}

fn expect_text<D>(kind: &str, value: Value, path: &str) -> Result<String, crate::DecodeFailure<D>> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(malformed(kind, path, "text")),
    }
}

fn expect_uint<D>(kind: &str, value: &Value, path: &str) -> Result<u64, crate::DecodeFailure<D>> {
    match value {
        Value::Uint(value) => Ok(*value),
        _ => Err(malformed(kind, path, "integer")),
    }
}

fn expect_bytes<D>(
    kind: &str,
    value: Value,
    path: &str,
) -> Result<Vec<u8>, crate::DecodeFailure<D>> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(malformed(kind, path, "byte string")),
    }
}

fn expect_digest<D>(
    kind: &str,
    value: Value,
    path: &str,
) -> Result<[u8; 32], crate::DecodeFailure<D>> {
    fixed_bytes(kind, value, path)
}

fn expect_key<D>(kind: &str, value: Value, path: &str) -> Result<u128, crate::DecodeFailure<D>> {
    let bytes: [u8; 16] = fixed_bytes(kind, value, path)?;
    Ok(u128::from_be_bytes(bytes))
}

fn fixed_bytes<D, const N: usize>(
    kind: &str,
    value: Value,
    path: &str,
) -> Result<[u8; N], crate::DecodeFailure<D>> {
    match value {
        Value::Bytes(bytes) if bytes.len() == N => {
            let mut raw = [0; N];
            raw.copy_from_slice(&bytes);
            Ok(raw)
        }
        Value::Bytes(bytes) => Err(canonical(
            kind,
            CanonicalFault::Noncanonical,
            path,
            "wrong_fixed_length",
            bytes.len().to_string(),
        )),
        _ => Err(malformed(kind, path, "byte string")),
    }
}

fn expect_variant<D>(
    kind: &str,
    value: Value,
    path: &str,
) -> Result<(String, Value), crate::DecodeFailure<D>> {
    let mut items = expect_array(kind, value, path)?;
    if items.len() != 2 {
        return Err(malformed(kind, path, "variant length"));
    }
    let name = expect_text(kind, items.remove(0), path)?;
    Ok((name, items.remove(0)))
}

fn into_fields<D>(
    kind: &str,
    value: Value,
    path: &str,
) -> Result<BTreeMap<String, Value>, crate::DecodeFailure<D>> {
    let items = expect_array(kind, value, path)?;
    let mut fields = BTreeMap::new();
    let mut previous: Option<String> = None;
    for item in items {
        let mut pair = expect_array(kind, item, path)?;
        if pair.len() != 2 {
            return Err(malformed(kind, path, "field length"));
        }
        let name = expect_text(kind, pair.remove(0), path)?;
        if let Some(previous) = &previous {
            if name.as_str() < previous.as_str() {
                return Err(canonical(
                    kind,
                    CanonicalFault::Noncanonical,
                    path,
                    "unsorted_field",
                    name,
                ));
            }
            if name == *previous {
                return Err(canonical(
                    kind,
                    CanonicalFault::Noncanonical,
                    path,
                    "duplicate_field",
                    name,
                ));
            }
        }
        previous = Some(name.clone());
        fields.insert(name, pair.remove(0));
    }
    Ok(fields)
}

fn reject_unknown<D>(
    kind: &str,
    fields: &BTreeMap<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), crate::DecodeFailure<D>> {
    for name in fields.keys() {
        if !allowed.contains(&name.as_str()) {
            return Err(unknown_field(kind, path, name));
        }
    }
    Ok(())
}

fn take_required<D>(
    kind: &str,
    fields: &mut BTreeMap<String, Value>,
    name: &str,
    path: &str,
) -> Result<Value, crate::DecodeFailure<D>> {
    fields
        .remove(name)
        .ok_or_else(|| malformed(kind, path, format!("missing {name}")))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::NetworkBuilder;
    use crate::cbor_decode::Value;
    use crate::diagnostics::ProblemEvidence;
    use crate::key::{ExternalInputKey, ExternalOutputKey, InPortKey, NodeKey, OutPortKey};
    use crate::metadata::DiagnosticMeta;
    use crate::policy::RuntimePolicy;
    use crate::signal::Level;
    use crate::time::Time;

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

    fn limits(frames: u64) -> DecodePolicy {
        DecodePolicy::new(
            8_000_000, 64, 2_000_000, 8_000_000, 200_000, 10_000, 10_000, 10_000, 1_000, 10_000,
            100_000, 100_000, 10_000, frames, 1_000_000,
        )
    }

    struct Fixture {
        compiled: crate::CompiledNetwork<Domain>,
        input: ExternalInputKey<Level>,
    }

    fn fixture() -> Fixture {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(70), TimeDomainId::from_u128(71));
        let input = ExternalInputKey::from_u128(1);
        let signal = builder
            .add_level_input(input, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("input must author: {failure:?}"));
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
        builder
            .add_level_output(
                ExternalOutputKey::from_u128(5),
                inverted,
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("output must author: {failure:?}"));
        let compiled = builder
            .finish()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
            .compile()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"));
        Fixture { compiled, input }
    }

    fn snapshot(fixture: &Fixture, level: LogicLevel) -> InputSnapshot<Domain> {
        fixture
            .compiled
            .input_snapshot()
            .set(fixture.input, level)
            .unwrap_or_else(|failure| panic!("observation must bind: {failure}"))
            .finish()
            .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"))
    }

    fn delta(fixture: &Fixture, level: LogicLevel) -> InputDelta<Domain> {
        fixture
            .compiled
            .input_delta()
            .set(fixture.input, level)
            .unwrap_or_else(|failure| panic!("delta must bind: {failure}"))
            .finish()
            .unwrap_or_else(|failure| panic!("delta must finish: {failure}"))
    }

    fn transactions(fixture: &Fixture) -> [Transaction<Domain>; 3] {
        let revision = fixture.compiled.spawn(policy()).revision();
        [
            Transaction::initialize(
                Time::from_ticks(0),
                revision,
                snapshot(fixture, LogicLevel::High),
            ),
            Transaction::advance(
                Time::from_ticks(1),
                revision,
                delta(fixture, LogicLevel::Low),
            ),
            Transaction::advance(
                Time::from_ticks(2),
                revision,
                delta(fixture, LogicLevel::High),
            ),
        ]
    }

    fn recorded() -> (Fixture, Machine<Domain>, ReplayLog<Domain>) {
        let fixture = fixture();
        let mut machine = fixture.compiled.spawn(policy());
        let log = record_replay_log(&mut machine, transactions(&fixture))
            .unwrap_or_else(|failure| panic!("recording must apply: {failure}"));
        (fixture, machine, log)
    }

    fn context(fixture: &Fixture) -> PersistenceContext<Domain> {
        PersistenceContext::new(fixture.compiled.time_domain_id())
    }

    fn code(failure: &ReplayFailure<Domain>) -> &'static str {
        failure.code().as_str()
    }

    fn version_evidence(failure: &ReplayFailure<Domain>) -> VersionView {
        match failure.problem().evidence() {
            ProblemEvidence::PersistenceUnsupportedVersion { evidence, .. } => VersionView {
                stage: evidence.stage,
                component: evidence.component.clone(),
                encountered: evidence.encountered.clone(),
                required: evidence.required.clone(),
                upgrader_exists: evidence.upgrader_exists,
            },
            other => panic!("expected unsupported version, got {other:?}"),
        }
    }

    struct VersionView {
        stage: &'static str,
        component: String,
        encountered: String,
        required: String,
        upgrader_exists: bool,
    }

    fn canonical_detail(failure: &ReplayFailure<Domain>) -> (String, String) {
        match failure.problem().evidence() {
            ProblemEvidence::PersistenceMalformedEnvelope { evidence, .. }
            | ProblemEvidence::PersistenceNoncanonicalEncoding { evidence, .. } => {
                (evidence.violation.to_owned(), evidence.encountered.clone())
            }
            ProblemEvidence::PersistenceUnknownSchemaField { evidence, .. } => (
                "unknown_schema_field".to_owned(),
                evidence.encountered.clone(),
            ),
            other => panic!("expected canonical or schema evidence, got {other:?}"),
        }
    }

    fn insert_field(payload: &[u8], name: &str, value: Value) -> Vec<u8> {
        let (mut parsed, consumed) = cbor_decode::parse(
            payload,
            Limits {
                total_bytes: 8_000_000,
                nesting: 64,
                text_bytes: 2_000_000,
                byte_string_bytes: 8_000_000,
                collection_items: 200_000,
            },
        )
        .unwrap_or_else(|failure| panic!("payload must parse: {failure:?}"));
        assert_eq!(consumed, payload.len());
        let Value::Array(items) = &mut parsed else {
            panic!("payload must be a record");
        };
        let extra = Value::Array(vec![Value::Text(name.to_owned()), value]);
        let mut index = items.len();
        for (position, item) in items.iter().enumerate() {
            let Value::Array(pair) = item else {
                continue;
            };
            let Value::Text(field) = &pair[0] else {
                continue;
            };
            if field.as_str() > name {
                index = position;
                break;
            }
        }
        items.insert(index, extra);
        cbor_decode::encode(&parsed)
    }

    fn seal_version(kind: &str, domain: TimeDomainId, payload: &[u8], version: u64) -> Vec<u8> {
        let bare = envelope_at(kind, domain, payload, None, version);
        let integrity =
            *blake3::hash(&domain_separated(ARTIFACT_INTEGRITY_DOMAIN, 2, &bare)).as_bytes();
        let signed = envelope_at(kind, domain, payload, Some(integrity), version);
        standalone(&signed)
    }

    #[test]
    fn frame_indexes_classify_missing_reordered_and_duplicated_work() {
        let (fixture, done, mut log) = recorded();
        log.frames[1].frame_index = 2;
        let mut machine = fixture.compiled.spawn(policy());
        let missing = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&missing), "replay.frame_missing");
        assert_eq!(missing.frame_index(), Some(2));
        assert!(machine.is_initialized());
        assert_eq!(machine.execution_state_digest(), {
            let mut first = fixture.compiled.spawn(policy());
            first.apply(transactions(&fixture)[0].clone()).unwrap();
            first.execution_state_digest()
        });

        let (_, _, mut log) = recorded();
        log.frames[0].frame_index = 1;
        log.frames[1].frame_index = 0;
        let mut machine = fixture.compiled.spawn(policy());
        let reordered = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&reordered), "replay.frame_reordered");
        assert!(!machine.is_initialized());

        let (_, _, mut log) = recorded();
        log.frames[1].frame_index = 0;
        let mut machine = fixture.compiled.spawn(policy());
        let duplicated = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&duplicated), "replay.frame_duplicated");
        assert!(machine.is_initialized());

        let (_, _, mut log) = recorded();
        log.frames[1].frame_index = 2;
        log.frames[2].frame_index = 1;
        let mut machine = fixture.compiled.spawn(policy());
        let filled = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&filled), "replay.frame_reordered");
        assert_eq!(machine.execution_state_digest(), {
            let mut first = fixture.compiled.spawn(policy());
            first.apply(transactions(&fixture)[0].clone()).unwrap();
            first.execution_state_digest()
        });
        let _ = done;
    }

    #[test]
    fn digest_and_linkage_failures_stop_at_the_named_frame() {
        let (fixture, done, log) = recorded();
        let mut frames = log.frames.clone();
        frames[0].expected_previous_execution_digest = ExecutionStateDigest::from_digest([9; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let starting = machine.replay(&frames).unwrap_err();
        assert_eq!(code(&starting), "replay.starting_execution_digest_mismatch");
        assert!(!machine.is_initialized());

        frames[0].expected_previous_execution_digest =
            log.frames[0].expected_previous_execution_digest;
        frames[1].expected_previous_execution_digest = ExecutionStateDigest::from_digest([9; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let later = machine.replay(&frames).unwrap_err();
        assert_eq!(code(&later), "replay.frame_reordered");
        assert_eq!(later.frame_index(), Some(1));
        assert!(machine.is_initialized());

        let mut linked = log.clone();
        linked.frames[0].expected_previous_execution_digest =
            ExecutionStateDigest::from_digest([4; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let linkage = machine.replay_log(&linked).unwrap_err();
        assert_eq!(code(&linkage), "replay.frame_reordered");
        assert!(!machine.is_initialized());

        let mut tampered = log.clone();
        tampered.frames[0].resulting_execution_digest = ExecutionStateDigest::from_digest([3; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let execution = machine.replay_log(&tampered).unwrap_err();
        assert_eq!(
            code(&execution),
            "replay.resulting_execution_digest_mismatch"
        );
        assert_eq!(machine.execution_state_digest(), {
            let mut first = fixture.compiled.spawn(policy());
            first.apply(transactions(&fixture)[0].clone()).unwrap();
            first.execution_state_digest()
        });

        let mut tampered = log.clone();
        tampered.frames[0].resulting_observable_digest =
            ObservableStateDigest::from_digest([3; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let observable = machine.replay_log(&tampered).unwrap_err();
        assert_eq!(
            code(&observable),
            "replay.resulting_observable_digest_mismatch"
        );
        assert_eq!(machine.observable_state_digest(), {
            let mut first = fixture.compiled.spawn(policy());
            first.apply(transactions(&fixture)[0].clone()).unwrap();
            first.observable_state_digest()
        });

        let mut finals = log.clone();
        finals.final_execution_digest = ExecutionStateDigest::from_digest([6; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let final_execution = machine.replay_log(&finals).unwrap_err();
        assert_eq!(
            code(&final_execution),
            "replay.resulting_execution_digest_mismatch"
        );
        assert_eq!(
            machine.execution_state_digest(),
            done.execution_state_digest()
        );

        let mut finals = log.clone();
        finals.final_observable_digest = ObservableStateDigest::from_digest([6; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let final_observable = machine.replay_log(&finals).unwrap_err();
        assert_eq!(
            code(&final_observable),
            "replay.resulting_observable_digest_mismatch"
        );
        assert_eq!(
            machine.observable_state_digest(),
            done.observable_state_digest()
        );

        let mut finals = log;
        finals.final_revision = NetworkRevision::from_value(9);
        let mut machine = fixture.compiled.spawn(policy());
        let revision = machine.replay_log(&finals).unwrap_err();
        assert_eq!(code(&revision), "replay.expected_revision_mismatch");
        assert_eq!(machine.revision(), done.revision());
    }

    #[test]
    fn checkpoint_observable_revision_and_per_frame_policy_are_distinct() {
        let (fixture, _, mut log) = recorded();
        log.starting_observable_digest = ObservableStateDigest::from_digest([8; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let observable = machine.replay_log(&log).unwrap_err();
        assert_eq!(
            code(&observable),
            "replay.starting_observable_digest_mismatch"
        );
        assert!(!machine.is_initialized());

        let (_, _, mut log) = recorded();
        log.starting_revision = NetworkRevision::from_value(4);
        let mut machine = fixture.compiled.spawn(policy());
        let revision = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&revision), "replay.expected_revision_mismatch");

        let (_, _, mut log) = recorded();
        log.frames[1].runtime_policy_id = RuntimePolicyId::from_digest([1; 32]);
        let mut machine = fixture.compiled.spawn(policy());
        let policy_failure = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&policy_failure), "replay.runtime_policy_mismatch");
        assert_eq!(policy_failure.frame_index(), Some(1));
        assert!(machine.is_initialized());

        let (_, _, mut log) = recorded();
        log.frames[1].expected_revision = NetworkRevision::from_value(9);
        let mut machine = fixture.compiled.spawn(policy());
        let frame_revision = machine.replay_log(&log).unwrap_err();
        assert_eq!(code(&frame_revision), "replay.expected_revision_mismatch");
        assert_eq!(frame_revision.frame_index(), Some(1));

        let (fixture, _, _) = recorded();
        let mut fresh = fixture.compiled.spawn(policy());
        let result = fresh
            .apply_recorded(transactions(&fixture)[0].clone())
            .unwrap_or_else(|failure| panic!("recorded init must apply: {failure}"))
            .into_result();
        let wrong = Transaction::initialize(
            Time::from_ticks(0),
            NetworkRevision::from_value(3),
            snapshot(&fixture, LogicLevel::High),
        );
        let failure = ReplayFrame::from_success(
            wrong,
            &result,
            fresh.runtime_policy_id(),
            fixture.compiled.time_domain_id(),
        )
        .unwrap_err();
        assert_eq!(code(&failure), "replay.expected_revision_mismatch");
    }

    #[test]
    fn swapped_frame_order_changes_bytes_and_semantic_versions_do_not_concatenate() {
        let (fixture, _, log) = recorded();
        let original = encoded_log(&log).bytes;
        let mut swapped = log.clone();
        swapped.frames.swap(0, 1);
        assert_ne!(encoded_log(&swapped).bytes, original);

        let mut machine = fixture.compiled.spawn(policy());
        let prefix =
            record_replay_log(&mut machine, transactions(&fixture)[..1].iter().cloned()).unwrap();
        let suffix =
            record_replay_log(&mut machine, transactions(&fixture)[1..].iter().cloned()).unwrap();
        let mut changed = suffix.clone();
        changed.versions.artifact_schema_version = 3;
        assert_eq!(
            code(&prefix.concatenate(&changed).unwrap_err()),
            "replay.logs_not_concatenable"
        );
        let context = context(&fixture);
        let bytes = encode_replay_log(&context, &changed).unwrap();
        let decoded = decode_replay_log(&context, bytes.as_bytes(), &limits(8)).unwrap_err();
        let version = version_evidence(&decoded);
        assert_eq!(version.stage, "payload");
        assert_eq!(version.component, "artifact_schema_version");
        assert_eq!(version.encountered, "3");
        assert_eq!(version.required, "2");
        assert!(!version.upgrader_exists);

        let envelope = seal_version("replay_log", log.time_domain, &log_payload(&log), 3);
        let decoded = decode_replay_log(&context, &envelope, &limits(8)).unwrap_err();
        let version = version_evidence(&decoded);
        assert_eq!(version.stage, "envelope");
        assert_eq!(version.component, "artifact_schema_version");
        assert_eq!(version.encountered, "3");
        assert_eq!(version.required, "2");
    }

    #[test]
    fn hostile_payloads_reject_unknown_fields_patch_results_and_pulse_counts() {
        let (fixture, _, log) = recorded();
        let context = context(&fixture);
        let frame = &log.frames[0];
        let extra = insert_field(
            &frame_payload(frame),
            "metadata",
            Value::Text("note".to_owned()),
        );
        let bytes = seal("replay_frame", frame.time_domain, &extra).bytes;
        let unknown = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&unknown), "persistence.unknown_schema_field");
        assert_eq!(canonical_detail(&unknown).1, "metadata");

        let child = seal(
            "transaction_result",
            frame.time_domain,
            &Record::new().finish(),
        )
        .bytes;
        let with_result = insert_field(
            &frame_payload(frame),
            "transaction_result",
            Value::Bytes(child),
        );
        let bytes = seal("replay_frame", frame.time_domain, &with_result).bytes;
        let result = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        let version = version_evidence(&result);
        assert_eq!(code(&result), "persistence.unsupported_version");
        assert_eq!(version.component, "artifact_schema_version");
        assert_eq!(version.encountered, "2");
        assert_eq!(version.required, "none");
        assert!(!version.upgrader_exists);
        assert_eq!(version.stage, "payload");

        let garbage = insert_field(
            &frame_payload(frame),
            "transaction_result",
            Value::Bytes(vec![0x01]),
        );
        let bytes = seal("replay_frame", frame.time_domain, &garbage).bytes;
        let garbage = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&garbage), "persistence.invalid_prefix");

        let input = frame.transaction.initialization_input().unwrap();
        let mut pulses = Cbor::default();
        pulses.array_start(1);
        pulses.array_start(2);
        pulses.bytes(&1u128.to_be_bytes());
        pulses.uint(0);
        let mut record = Record::new();
        record.field("input_schema_fingerprint", |writer| {
            writer.bytes(&input.input_schema_fingerprint().as_bytes());
        });
        record.field("levels", |writer| write_levels(writer, &input.levels));
        record.field("network_fingerprint", |writer| {
            writer.bytes(&input.network_fingerprint().as_bytes());
        });
        record.field("network_key", |writer| {
            writer.bytes(&input.network_key().as_u128().to_be_bytes());
        });
        record.field("pulses", |writer| writer.nested(&pulses.finish()));
        let input_bytes = seal("input_snapshot", frame.time_domain, &record.finish()).bytes;
        let transaction = transaction_with_input(frame, "input_snapshot", &input_bytes);
        let payload = frame_with_transaction(frame, &transaction);
        let bytes = seal("replay_frame", frame.time_domain, &payload).bytes;
        let zero = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&zero), "persistence.malformed_envelope");
        assert_eq!(canonical_detail(&zero).1, "non_positive");

        let mut levels = Cbor::default();
        levels.array_start(2);
        levels.array_start(2);
        levels.bytes(&2u128.to_be_bytes());
        levels.variant_null("low");
        levels.array_start(2);
        levels.bytes(&1u128.to_be_bytes());
        levels.variant_null("high");
        let mut record = Record::new();
        record.field("input_schema_fingerprint", |writer| {
            writer.bytes(&input.input_schema_fingerprint().as_bytes());
        });
        record.field("levels", |writer| writer.nested(&levels.finish()));
        record.field("network_fingerprint", |writer| {
            writer.bytes(&input.network_fingerprint().as_bytes());
        });
        record.field("network_key", |writer| {
            writer.bytes(&input.network_key().as_u128().to_be_bytes());
        });
        record.field("pulses", |writer| write_pulses(writer, &input.pulses));
        let input_bytes = seal("input_snapshot", frame.time_domain, &record.finish()).bytes;
        let transaction = transaction_with_input(frame, "input_snapshot", &input_bytes);
        let bytes = seal(
            "replay_frame",
            frame.time_domain,
            &frame_with_transaction(frame, &transaction),
        )
        .bytes;
        let unsorted = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&unsorted), "persistence.noncanonical_encoding");
        assert_eq!(canonical_detail(&unsorted).0, "unsorted_key");

        let patched = transaction_with_patch(frame);
        let bytes = seal(
            "replay_frame",
            frame.time_domain,
            &frame_with_transaction(frame, &patched),
        )
        .bytes;
        let patch = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&patch), "replay.patch_preparation_diverged");

        let foreign_input = seal(
            "input_snapshot",
            TimeDomainId::from_u128(0),
            &input_payload(
                input.network_key(),
                input.network_fingerprint(),
                input.input_schema_fingerprint(),
                &input.levels,
                &input.pulses,
            ),
        )
        .bytes;
        let transaction = transaction_with_input(frame, "input_snapshot", &foreign_input);
        let bytes = seal(
            "replay_frame",
            frame.time_domain,
            &frame_with_transaction(frame, &transaction),
        )
        .bytes;
        let domain = decode_replay_frame(&context, &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&domain), "persistence.wrong_time_domain");

        let wrong_kind = seal(
            "machine_snapshot",
            frame.time_domain,
            &Record::new().finish(),
        )
        .bytes;
        let recognized = decode_replay_log(&context, &wrong_kind, &limits(8)).unwrap_err();
        assert_eq!(code(&recognized), "persistence.malformed_envelope");
        let unknown_kind = seal("not_a_kind", frame.time_domain, &Record::new().finish()).bytes;
        let unknown = decode_replay_log(&context, &unknown_kind, &limits(8)).unwrap_err();
        assert_eq!(code(&unknown), "persistence.unknown_artifact_kind");
    }

    fn transaction_with_input(frame: &ReplayFrame<Domain>, kind: &str, input: &[u8]) -> Vec<u8> {
        let snapshot = frame.transaction.initialization_input().unwrap();
        let mut record = Record::new();
        record.field("expected_revision", |writer| {
            writer.uint(frame.transaction.expected_revision().value());
        });
        record.field("input", |writer| {
            writer.variant_start(kind);
            writer.bytes(input);
        });
        record.field("input_schema_fingerprint", |writer| {
            writer.bytes(&snapshot.input_schema_fingerprint().as_bytes());
        });
        record.field("network_fingerprint", |writer| {
            writer.bytes(&snapshot.network_fingerprint().as_bytes());
        });
        record.field("network_key", |writer| {
            writer.bytes(&snapshot.network_key().as_u128().to_be_bytes());
        });
        record.field("requested_time", |writer| {
            writer.uint(frame.transaction.requested_time().ticks());
        });
        seal("transaction_record", frame.time_domain, &record.finish()).bytes
    }

    fn transaction_with_patch(frame: &ReplayFrame<Domain>) -> Vec<u8> {
        let encoded = transaction_with_input(
            frame,
            "input_snapshot",
            &encoded_input(&frame.transaction, frame.time_domain)
                .encoded
                .bytes,
        );
        let opened = open_artifact::<Domain>(&encoded, &limits(8), "transaction_record").unwrap();
        let payload = cbor_decode::encode(&opened.payload);
        let patched = insert_field(&payload, "patch", Value::Bytes(Vec::new()));
        seal("transaction_record", frame.time_domain, &patched).bytes
    }

    fn frame_with_transaction(frame: &ReplayFrame<Domain>, transaction: &[u8]) -> Vec<u8> {
        let mut record = Record::new();
        record.field("expected_previous_execution_digest", |writer| {
            writer.bytes(&frame.expected_previous_execution_digest.as_bytes());
        });
        record.field("expected_revision", |writer| {
            writer.uint(frame.expected_revision.value())
        });
        record.field("frame_index", |writer| writer.uint(frame.frame_index));
        record.field("resulting_execution_digest", |writer| {
            writer.bytes(&frame.resulting_execution_digest.as_bytes());
        });
        record.field("resulting_observable_digest", |writer| {
            writer.bytes(&frame.resulting_observable_digest.as_bytes());
        });
        record.field("runtime_policy_id", |writer| {
            writer.bytes(&frame.runtime_policy_id.as_bytes());
        });
        record.field("transaction_record", |writer| writer.bytes(transaction));
        record.finish()
    }

    #[test]
    fn materialization_preserves_input_codes_and_leaves_the_machine() {
        let (fixture, _, log) = recorded();
        let frame = &log.frames[0];
        let input = frame.transaction.initialization_input().unwrap();
        let mut foreign = frame.clone();
        foreign.transaction = Transaction::initialize(
            frame.transaction.requested_time(),
            frame.transaction.expected_revision(),
            InputSnapshot::from_decoded(
                input.network_key(),
                NetworkFingerprint::from_digest([9; 32]),
                input.input_schema_fingerprint(),
                input.levels.clone(),
                input.pulses.clone(),
            ),
        );
        let mut machine = fixture.compiled.spawn(policy());
        let fingerprint = machine.replay(std::slice::from_ref(&foreign)).unwrap_err();
        assert_eq!(code(&fingerprint), "replay.network_fingerprint_mismatch");
        assert!(!machine.is_initialized());

        let mut wrong_key = frame.clone();
        wrong_key.transaction = Transaction::initialize(
            frame.transaction.requested_time(),
            frame.transaction.expected_revision(),
            InputSnapshot::from_decoded(
                NetworkKey::from_u128(999),
                input.network_fingerprint(),
                input.input_schema_fingerprint(),
                input.levels.clone(),
                input.pulses.clone(),
            ),
        );
        let key = machine
            .replay(std::slice::from_ref(&wrong_key))
            .unwrap_err();
        assert_eq!(code(&key), "input.wrong_network");
        assert!(!machine.is_initialized());

        let mut wrong_schema = frame.clone();
        wrong_schema.transaction = Transaction::initialize(
            frame.transaction.requested_time(),
            frame.transaction.expected_revision(),
            InputSnapshot::from_decoded(
                input.network_key(),
                input.network_fingerprint(),
                InputSchemaFingerprint::from_digest([4; 32]),
                input.levels.clone(),
                input.pulses.clone(),
            ),
        );
        let schema = machine
            .replay(std::slice::from_ref(&wrong_schema))
            .unwrap_err();
        assert_eq!(code(&schema), "input.foreign_schema");

        let mut missing = frame.clone();
        missing.transaction = Transaction::initialize(
            frame.transaction.requested_time(),
            frame.transaction.expected_revision(),
            InputSnapshot::from_decoded(
                input.network_key(),
                input.network_fingerprint(),
                input.input_schema_fingerprint(),
                BTreeMap::new(),
                BTreeMap::new(),
            ),
        );
        let omitted = machine.replay(std::slice::from_ref(&missing)).unwrap_err();
        assert_eq!(code(&omitted), "input.missing_required_level");
        assert_eq!(omitted.frame_index(), Some(0));
        assert_eq!(omitted.logical_time(), Some(0));
        assert!(!machine.is_initialized());
    }

    #[test]
    fn a_frame_inside_a_log_must_use_the_log_time_domain() {
        let (fixture, _, mut log) = recorded();
        log.frames[0].time_domain = TimeDomainId::from_u128(0);
        let bytes = encoded_log(&log).bytes;
        let failure = decode_replay_log(&context(&fixture), &bytes, &limits(8)).unwrap_err();
        assert_eq!(code(&failure), "persistence.wrong_time_domain");
    }
}
