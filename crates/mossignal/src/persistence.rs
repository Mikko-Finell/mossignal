//! Canonical machine-snapshot artifacts.
//!
//! Encoding produces one prefixed CBOR envelope. Decoding and restoration use
//! that same envelope and do not own filesystem access.

use crate::compile::{SnapshotNode, SnapshotNodeFamily};
use crate::identity::{
    Cbor, ExecutionStateDigest, NetworkFingerprint, ObservableStateDigest, SNAPSHOT_DIGEST_DOMAIN,
    SnapshotDigest, TimeDomainId, domain_separated, logic_level,
};
use crate::key::NodeKey;
use crate::machine::{Machine, MachineStatus, NetworkRevision, PendingEvent};
use crate::policy::RuntimePolicyId;
use crate::signal::LogicLevel;
use crate::state_digest::{
    CauseDigestIndex, cause_digest_index, encode_episode_evidence, encode_node_subject,
    encode_settled_endpoint, encode_stable_owner, episode_evidence_revision,
};
use crate::time::Time;
use crate::{ActiveDiagnosticEpisode, EdgeObservation};
use core::fmt;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};

// SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "standalone-framing"
// The eight prefix bytes frame the artifact and are not a digest input.
const ARTIFACT_PREFIX: [u8; 8] = [0x4d, 0x53, 0x49, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const SEMANTIC_VERSION: u64 = 2;

/// Caller-owned time-domain binding checked before snapshot bytes are emitted.
#[derive(Debug)]
pub struct PersistenceContext<D> {
    time_domain: TimeDomainId,
    domain: PhantomData<fn() -> D>,
}

impl<D> PersistenceContext<D> {
    /// Binds encoding to one caller-owned logical time domain.
    #[must_use]
    pub const fn new(time_domain: TimeDomainId) -> Self {
        Self {
            time_domain,
            domain: PhantomData,
        }
    }

    /// Returns the time domain this context will accept.
    #[must_use]
    pub const fn time_domain(&self) -> TimeDomainId {
        self.time_domain
    }
}

impl<D> Clone for PersistenceContext<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for PersistenceContext<D> {}

impl<D> PartialEq for PersistenceContext<D> {
    fn eq(&self, other: &Self) -> bool {
        self.time_domain == other.time_domain
    }
}

impl<D> Eq for PersistenceContext<D> {}

/// Owned canonical bytes of one standalone artifact, including its fixed prefix.
#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactBytes(Vec<u8>);

impl ArtifactBytes {
    /// Returns the prefixed canonical artifact.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn from_canonical(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for ArtifactBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Debug for ArtifactBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArtifactBytes")
            .field("len", &self.0.len())
            .finish()
    }
}

/// Failure to emit a canonical artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeFailure {
    /// The persistence context names a different time domain than the snapshot.
    TimeDomainMismatch,
}

impl fmt::Display for EncodeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeDomainMismatch => formatter.write_str(
                "persistence context time domain does not match the snapshot time domain",
            ),
        }
    }
}

impl std::error::Error for EncodeFailure {}

/// Owned canonical snapshot of one committed machine version.
#[derive(Clone, PartialEq, Eq)]
pub struct MachineSnapshot<D> {
    status: MachineStatus<D>,
    revision: NetworkRevision,
    fingerprint: NetworkFingerprint,
    execution: ExecutionStateDigest,
    observable: ObservableStateDigest,
    snapshot: SnapshotDigest,
    policy: RuntimePolicyId,
    time_domain: TimeDomainId,
    bytes: Vec<u8>,
}

impl<D> MachineSnapshot<D> {
    /// Returns the lifecycle state captured by this snapshot.
    #[must_use]
    pub const fn status(&self) -> MachineStatus<D> {
        self.status
    }

    /// Returns the installed topology revision.
    #[must_use]
    pub const fn revision(&self) -> NetworkRevision {
        self.revision
    }

    /// Returns the installed network fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> NetworkFingerprint {
        self.fingerprint
    }

    /// Returns the execution-state digest embedded in the artifact.
    #[must_use]
    pub const fn execution_state_digest(&self) -> ExecutionStateDigest {
        self.execution
    }

    /// Returns the observable-state digest embedded in the artifact.
    #[must_use]
    pub const fn observable_state_digest(&self) -> ObservableStateDigest {
        self.observable
    }

    /// Returns the envelope integrity digest.
    #[must_use]
    pub const fn snapshot_digest(&self) -> SnapshotDigest {
        self.snapshot
    }

    /// Returns the runtime-policy identity recorded by the artifact.
    #[must_use]
    pub const fn runtime_policy_id(&self) -> RuntimePolicyId {
        self.policy
    }

    pub(crate) fn artifact_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_decoded(
        status: MachineStatus<D>,
        revision: NetworkRevision,
        fingerprint: NetworkFingerprint,
        execution: ExecutionStateDigest,
        observable: ObservableStateDigest,
        snapshot: SnapshotDigest,
        policy: RuntimePolicyId,
        time_domain: TimeDomainId,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            status,
            revision,
            fingerprint,
            execution,
            observable,
            snapshot,
            policy,
            time_domain,
            bytes,
        }
    }
}

impl<D> fmt::Debug for MachineSnapshot<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MachineSnapshot")
            .field("status", &self.status)
            .field("revision", &self.revision)
            .field("fingerprint", &self.fingerprint)
            .field("execution_state_digest", &self.execution)
            .field("observable_state_digest", &self.observable)
            .field("snapshot_digest", &self.snapshot)
            .field("runtime_policy_id", &self.policy)
            .finish_non_exhaustive()
    }
}

/// Encodes a snapshot when the context time domain matches the artifact.
///
/// A mismatch returns [`EncodeFailure`] and does not produce bytes.
pub fn encode_snapshot<D>(
    context: &PersistenceContext<D>,
    snapshot: &MachineSnapshot<D>,
) -> Result<ArtifactBytes, EncodeFailure> {
    if context.time_domain != snapshot.time_domain {
        return Err(EncodeFailure::TimeDomainMismatch);
    }
    Ok(ArtifactBytes(snapshot.bytes.clone()))
}

pub(crate) fn snapshot_from_machine<D>(machine: &Machine<D>) -> MachineSnapshot<D> {
    project(machine, None)
}

#[cfg(test)]
fn snapshot_with_label<D>(machine: &Machine<D>, label: &str) -> MachineSnapshot<D> {
    project(machine, Some(label))
}

fn project<D>(machine: &Machine<D>, label: Option<&str>) -> MachineSnapshot<D> {
    assert_projection_invariants(machine);
    let execution = machine.execution_state_digest();
    let observable = machine.observable_state_digest();
    let payload = payload_bytes(machine, execution, observable, label);
    let time_domain = machine.compiled.time_domain_id();
    let bare = envelope(time_domain, &payload, None);
    // SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "snapshot-digest-identity"
    // SnapshotDigest hashes the envelope record with integrity_digest omitted.
    let digest_bytes =
        *blake3::hash(&domain_separated(SNAPSHOT_DIGEST_DOMAIN, 2, &bare)).as_bytes();
    let snapshot = SnapshotDigest::from_digest(digest_bytes);
    let full = envelope(time_domain, &payload, Some(digest_bytes));
    #[cfg(test)]
    crate::projection_work::without_accounting(|| {
        let reference_index = crate::state_digest_reference::artifact_index(machine);
        let reference_payload = payload_with_index(
            machine,
            crate::state_digest_reference::execution_state_digest(machine),
            crate::state_digest_reference::observable_state_digest(machine),
            label,
            &reference_index,
        );
        let reference_bare = envelope(time_domain, &reference_payload, None);
        let reference_digest = *blake3::hash(&domain_separated(
            SNAPSHOT_DIGEST_DOMAIN,
            2,
            &reference_bare,
        ))
        .as_bytes();
        assert_eq!(
            full,
            envelope(time_domain, &reference_payload, Some(reference_digest)),
            "full snapshot bytes versus uncached graph projection"
        );
    });
    MachineSnapshot {
        status: machine.status(),
        revision: machine.revision(),
        fingerprint: machine.fingerprint(),
        execution,
        observable,
        snapshot,
        policy: machine.policy.id(),
        time_domain,
        bytes: standalone(&full),
    }
}

fn assert_projection_invariants<D>(machine: &Machine<D>) {
    match machine.store.status {
        MachineStatus::AwaitingInitialization => {
            if !machine.store.pending_events.is_empty()
                || machine.last_reaction().is_some()
                || !machine.store.active_episodes.is_empty()
                || machine.store.provenance.is_some()
                || !machine.store.external_levels.is_empty()
                || !machine.store.output_baselines.is_empty()
                || !machine.store.periodic_anchors.is_empty()
                || !cause_maps_empty(machine)
            {
                panic!("uninitialized snapshot must not contain ready runtime facts");
            }
        }
        MachineStatus::Ready { now } => {
            if machine.store.provenance.is_none() {
                panic!("ready snapshot must retain its provenance view");
            }
            let next = machine.store.next_pending_event_serial;
            for event in machine.store.pending_events.values().flatten().copied() {
                let (key, _, _, deadline, _, _) = event.identity();
                if deadline.ticks() <= now.ticks() {
                    panic!("ready snapshot deadline must be strictly later than snapshot time");
                }
                if key.value() >= next {
                    panic!("next pending-event serial must exceed every retained public serial");
                }
            }
        }
    }
}

fn cause_maps_empty<D>(machine: &Machine<D>) -> bool {
    machine.store.input_causes.is_empty()
        && machine.store.output_causes.is_empty()
        && machine.store.edge_observation_causes.is_empty()
        && machine.store.toggle_inversion_causes.is_empty()
        && machine.store.establishment_causes.is_empty()
        && machine.store.transport_transition_causes.is_empty()
        && machine.store.inertial_cancellation_causes.is_empty()
        && machine.store.periodic_anchor_causes.is_empty()
        && machine.store.periodic_cancellation_causes.is_empty()
}

fn payload_bytes<D>(
    machine: &Machine<D>,
    execution: ExecutionStateDigest,
    observable: ObservableStateDigest,
    label: Option<&str>,
) -> Vec<u8> {
    let nodes = machine.compiled.snapshot_nodes();
    let kinds = node_kinds(&nodes);
    assert_event_kinds(machine, &kinds);
    let provenance = cause_digest_index(machine);
    payload_with_index(machine, execution, observable, label, &provenance)
}

fn payload_with_index<D>(
    machine: &Machine<D>,
    execution: ExecutionStateDigest,
    observable: ObservableStateDigest,
    label: Option<&str>,
    provenance: &CauseDigestIndex,
) -> Vec<u8> {
    let nodes = machine.compiled.snapshot_nodes();
    let mut record = Record::new();
    record.field("active_diagnostic_episodes", |writer| {
        write_episodes(writer, machine, provenance);
    });
    record.field("execution_state_digest", |writer| {
        writer.bytes(&execution.as_bytes());
    });
    record.field("lifecycle", |writer| {
        writer.nested(&lifecycle_bytes(machine, provenance));
    });
    record.field("network_fingerprint", |writer| {
        writer.bytes(&machine.fingerprint().as_bytes());
    });
    record.field("network_key", |writer| {
        writer.key(machine.compiled.network_key().as_u128());
    });
    record.field("next_pending_event_serial", |writer| {
        writer.uint(machine.store.next_pending_event_serial);
    });
    record.field("node_state_table", |writer| {
        write_state_table(writer, machine, &nodes, Table::Node);
    });
    record.field("observable_state_digest", |writer| {
        writer.bytes(&observable.as_bytes());
    });
    record.field("provenance", |writer| {
        write_provenance(writer, machine, provenance);
    });
    record.field("runtime_policy_id", |writer| {
        writer.bytes(&machine.policy.id().as_bytes());
    });
    record.field("semantic_versions", |writer| {
        writer.nested(&semantic_versions());
    });
    record.field("temporal_state_table", |writer| {
        write_state_table(writer, machine, &nodes, Table::Temporal);
    });
    record.field("time_domain_id", |writer| {
        writer.bytes(&machine.compiled.time_domain_id().to_be_bytes());
    });
    record.field("topology_revision", |writer| {
        writer.uint(machine.revision().value());
    });
    if let Some(label) = label {
        record.field("persistence_metadata", |writer| {
            let mut meta = Record::new();
            meta.field("label", |writer| writer.text(label));
            writer.nested(&meta.finish());
        });
    }
    record.finish()
}

fn node_kinds(nodes: &[SnapshotNode]) -> BTreeMap<NodeKey, &'static str> {
    nodes.iter().map(|node| (node.flat, node.kind)).collect()
}

fn assert_event_kinds<D>(machine: &Machine<D>, kinds: &BTreeMap<NodeKey, &'static str>) {
    for event in machine.store.pending_events.values().flatten().copied() {
        let expected = match event {
            PendingEvent::PulseDelay(_) => "pulse_delay",
            PendingEvent::TransportDelay(_) => "transport_delay",
            PendingEvent::Inertial(_) => "inertial_delay",
            PendingEvent::Periodic(_) => "periodic",
        };
        let (_, node, _, _, _, _) = event.identity();
        match kinds.get(&node).copied() {
            Some(actual) if actual == expected => {}
            _ => panic!("pending event kind must agree with the owning node kind"),
        }
    }
}

fn semantic_versions() -> Vec<u8> {
    let mut record = Record::new();
    version(&mut record, "core_semantics_version");
    version(&mut record, "diagnostic_schema_version");
    version(&mut record, "node_semantics_version");
    version(&mut record, "patch_semantics_version");
    version(&mut record, "provenance_semantics_version");
    record.finish()
}

fn version(record: &mut Record, name: &'static str) {
    record.field(name, |writer| {
        writer.uint(match name {
            "artifact_schema_version" | "provenance_semantics_version" => 3,
            _ => SEMANTIC_VERSION,
        })
    });
}

fn lifecycle_bytes<D>(machine: &Machine<D>, provenance: &CauseDigestIndex) -> Vec<u8> {
    let mut writer = Cbor::default();
    match machine.store.status {
        MachineStatus::AwaitingInitialization => {
            writer.variant_null("awaiting_initialization");
        }
        MachineStatus::Ready { now } => {
            writer.variant_start("ready");
            writer.nested(&ready_lifecycle(machine, provenance, now));
        }
    }
    writer.finish()
}

fn ready_lifecycle<D>(
    machine: &Machine<D>,
    provenance: &CauseDigestIndex,
    now: Time<D>,
) -> Vec<u8> {
    let mut record = Record::new();
    // SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "lifecycle-shape"
    // With no checkpoint, the ready explanation boundary is complete from initialization.
    record.field("explanation_boundary", |writer| {
        writer.variant_null("complete_from_initialization");
    });
    record.field("external_levels", |writer| {
        write_external_levels(writer, machine)
    });
    record.field("output_baselines", |writer| {
        write_baselines(writer, machine, provenance);
    });
    record.field("pending_events", |writer| {
        write_pending_events(writer, machine, provenance);
    });
    record.field("settled_levels", |writer| {
        write_settled_levels(writer, machine)
    });
    let ticks = now.ticks();
    record.field("time", |writer| writer.uint(ticks));
    record.field("reaction_order", |writer| {
        let order = match machine.last_reaction() {
            Some(stamp) if stamp.time() == now => stamp.order(),
            _ => panic!("ready snapshot must retain its committed reaction stamp at current time"),
        };
        writer.uint(order)
    });
    record.finish()
}

fn write_external_levels<D>(writer: &mut Cbor, machine: &Machine<D>) {
    let mut levels: Vec<_> = machine
        .store
        .external_levels
        .iter()
        .map(|(key, level)| (key.as_u128(), *level))
        .collect();
    levels.sort_by_key(|(key, _)| *key);
    writer.array_start(levels.len());
    for (key, level) in levels {
        writer.array_start(2);
        writer.key(key);
        logic_level(writer, level);
    }
}

fn write_baselines<D>(writer: &mut Cbor, machine: &Machine<D>, provenance: &CauseDigestIndex) {
    let mut baselines = Vec::new();
    for key in machine.compiled.external_level_outputs() {
        let level = match machine.store.output_baselines.get(&key).copied() {
            Some(level) => level,
            None => panic!("ready external level output must have an established baseline"),
        };
        let cause = match machine.store.output_causes.get(&key).copied() {
            Some(cause) => cause_digest(machine, provenance, cause),
            None => panic!("ready external level output baseline must have a cause"),
        };
        let mut record = Record::new();
        record.field("cause", |writer| writer.bytes(&cause));
        record.field("established", |writer| writer.boolean(true));
        let raw = key.as_u128();
        record.field("key", |writer| writer.key(raw));
        record.field("level", |writer| logic_level(writer, level));
        baselines.push((raw, record.finish()));
    }
    baselines.sort_by_key(|(key, _)| *key);
    writer.array_start(baselines.len());
    for (_, baseline) in baselines {
        writer.nested(&baseline);
    }
}

fn write_settled_levels<D>(writer: &mut Cbor, machine: &Machine<D>) {
    let mut facts = Vec::new();
    for slot in machine.compiled.settled_level_slots() {
        let level = match machine
            .store
            .operation_levels
            .get(slot.operation)
            .copied()
            .flatten()
        {
            Some(level) => level,
            None => panic!("ready settled level fact must have a level value"),
        };
        let subject = encode_settled_endpoint(&slot.endpoint);
        facts.push((subject, level));
    }
    // SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "required-observation"
    // External level inputs stay settled facts alongside the authoritative valuation.
    for (key, level) in &machine.store.external_levels {
        let mut subject = Cbor::default();
        subject.variant_start("external_input");
        subject.key(key.as_u128());
        facts.push((subject.finish(), *level));
    }
    facts.sort_by(|left, right| left.0.cmp(&right.0));
    writer.array_start(facts.len());
    for (subject, level) in facts {
        let mut record = Record::new();
        record.field("level", |writer| logic_level(writer, level));
        record.field("subject", |writer| writer.nested(&subject));
        writer.nested(&record.finish());
    }
}

fn write_pending_events<D>(writer: &mut Cbor, machine: &Machine<D>, provenance: &CauseDigestIndex) {
    let mut events = Vec::new();
    for event in machine.store.pending_events.values().flatten().copied() {
        let (key, node, origin, deadline, revision, cause) = event.identity();
        let owner = encode_stable_owner(&machine.compiled.stable_owner(node));
        let cause = cause_digest(machine, provenance, cause);
        let (kind_name, kind) = pending_kind(event);
        let mut record = Record::new();
        record.field("cause", |writer| writer.bytes(&cause));
        let deadline = deadline.ticks();
        record.field("deadline", |writer| writer.uint(deadline));
        let serial = key.value();
        record.field("key", |writer| writer.uint(serial));
        record.field("kind", |writer| writer.nested(&kind));
        let revision = revision.value();
        record.field("origin_revision", |writer| writer.uint(revision));
        let origin = origin.ticks();
        record.field("origin_time", |writer| writer.uint(origin));
        record.field("stimulus_time", |writer| {
            writer.uint(event.stimulus().time().ticks())
        });
        record.field("stimulus_order", |writer| {
            writer.uint(event.stimulus().order())
        });
        record.field("owner", |writer| writer.nested(&owner));
        events.push((deadline, owner, kind_name, serial, record.finish()));
    }
    events.sort_by(|left, right| {
        (&left.0, &left.1, left.2, left.3).cmp(&(&right.0, &right.1, right.2, right.3))
    });
    writer.array_start(events.len());
    for (_, _, _, _, event) in events {
        writer.nested(&event);
    }
}

fn pending_kind<D>(event: PendingEvent<D>) -> (&'static str, Vec<u8>) {
    let mut writer = Cbor::default();
    let name = match event {
        PendingEvent::PulseDelay(event) => {
            writer.variant_start("pulse_delay_group");
            let mut body = Record::new();
            let count = event.count.get();
            body.field("count", |writer| writer.uint(count));
            writer.nested(&body.finish());
            "pulse_delay_group"
        }
        PendingEvent::TransportDelay(event) => {
            writer.variant_start("transport_transition");
            let mut body = Record::new();
            let origin = event.origin.ticks();
            body.field("origin_time", |writer| writer.uint(origin));
            body.field("target", |writer| logic_level(writer, event.target));
            writer.nested(&body.finish());
            "transport_transition"
        }
        PendingEvent::Inertial(event) => {
            writer.variant_start("inertial_maturation");
            let mut body = Record::new();
            let origin = event.origin.ticks();
            body.field("qualification_origin", |writer| writer.uint(origin));
            body.field("target", |writer| logic_level(writer, event.target));
            writer.nested(&body.finish());
            "inertial_maturation"
        }
        PendingEvent::Periodic(event) => {
            writer.variant_start("periodic_boundary");
            let mut body = Record::new();
            let anchor = event.anchor.ticks();
            let ordinal = event.ordinal;
            body.field("anchor", |writer| writer.uint(anchor));
            body.field("ordinal", |writer| writer.uint(ordinal));
            writer.nested(&body.finish());
            "periodic_boundary"
        }
    };
    (name, writer.finish())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Node,
    Temporal,
}

fn write_state_table<D>(
    writer: &mut Cbor,
    machine: &Machine<D>,
    nodes: &[SnapshotNode],
    table: Table,
) {
    let mut entries = Vec::new();
    for node in nodes {
        for (placed, schema, value) in node_facts(machine, node) {
            if placed != table {
                continue;
            }
            let owner = encode_stable_owner(&node.owner);
            let bytes = state_entry(&owner, node.kind, schema, &value);
            entries.push((owner, schema, bytes));
        }
    }
    entries.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
    writer.array_start(entries.len());
    for (_, _, entry) in entries {
        writer.nested(&entry);
    }
}

fn state_entry(owner: &[u8], kind: &str, schema: &str, value: &[u8]) -> Vec<u8> {
    let mut record = Record::new();
    record.field("node_kind", |writer| writer.variant_null(kind));
    record.field("owner", |writer| writer.nested(owner));
    record.field("schema", |writer| writer.variant_null(schema));
    record.field("value", |writer| writer.nested(value));
    record.finish()
}

fn node_facts<D>(machine: &Machine<D>, node: &SnapshotNode) -> Vec<(Table, &'static str, Vec<u8>)> {
    match node.family {
        SnapshotNodeFamily::Edge { index } => vec![(
            Table::Node,
            "edge_observation",
            edge_value(stored_edge(machine, index)),
        )],
        SnapshotNodeFamily::StoredLevel { index } => vec![(
            Table::Node,
            "stored_level",
            level_value(stored_level(machine, index)),
        )],
        SnapshotNodeFamily::PulseDelay => {
            vec![(Table::Temporal, "pending_pulse_group", null_value())]
        }
        SnapshotNodeFamily::Transport { remembered, output } => vec![
            (
                Table::Node,
                "remembered_input_output",
                remembered_value(machine, remembered, output),
            ),
            (
                Table::Temporal,
                "pending_transport_transition",
                null_value(),
            ),
        ],
        SnapshotNodeFamily::Inertial { remembered, output } => vec![
            (
                Table::Node,
                "remembered_input_output",
                remembered_value(machine, remembered, output),
            ),
            (
                Table::Temporal,
                "pending_inertial_candidate",
                singular_reference(machine, node.flat, "inertial_delay"),
            ),
        ],
        SnapshotNodeFamily::Periodic { previous_enable } => vec![
            (
                Table::Node,
                "periodic_anchor_previous_enable",
                periodic_value(machine, node.flat, previous_enable),
            ),
            (
                Table::Temporal,
                "pending_periodic_boundary",
                singular_reference(machine, node.flat, "periodic"),
            ),
        ],
    }
}

fn edge_value(observation: EdgeObservation) -> Vec<u8> {
    let mut writer = Cbor::default();
    match observation {
        EdgeObservation::Unestablished => writer.variant_null("unestablished"),
        EdgeObservation::Established(level) => {
            writer.variant_start("established");
            logic_level(&mut writer, level);
        }
    }
    writer.finish()
}

fn level_value(level: LogicLevel) -> Vec<u8> {
    let mut writer = Cbor::default();
    logic_level(&mut writer, level);
    writer.finish()
}

fn remembered_value<D>(machine: &Machine<D>, remembered: usize, output: usize) -> Vec<u8> {
    let mut record = Record::new();
    let output = stored_level(machine, output);
    let remembered = stored_level(machine, remembered);
    record.field("output", |writer| logic_level(writer, output));
    record.field("remembered_input", |writer| logic_level(writer, remembered));
    record.finish()
}

fn periodic_value<D>(machine: &Machine<D>, node: NodeKey, previous_enable: usize) -> Vec<u8> {
    let mut record = Record::new();
    if let Some(anchor) = machine.store.periodic_anchors.get(&node).copied() {
        let ticks = anchor.anchor.ticks();
        record.field("anchor", |writer| writer.uint(ticks));
        record.field("phase_time", |writer| {
            writer.uint(anchor.origin.time().ticks())
        });
        record.field("phase_order", |writer| writer.uint(anchor.origin.order()));
        if let Some(settled) = anchor.settled {
            record.field("settled_boundary", |writer| writer.uint(settled.ticks()));
        }
    }
    let previous = stored_level(machine, previous_enable);
    record.field("previous_enable", |writer| logic_level(writer, previous));
    record.finish()
}

fn singular_reference<D>(machine: &Machine<D>, node: NodeKey, kind: &str) -> Vec<u8> {
    match singular_serial(machine, node, kind) {
        Some(serial) => {
            let mut record = Record::new();
            record.field("event", |writer| writer.uint(serial));
            record.finish()
        }
        None => null_value(),
    }
}

// SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "state-and-pending-facts"
// Pulse-delay payloads stay in the calendar. Temporal state does not copy them.
fn null_value() -> Vec<u8> {
    let mut writer = Cbor::default();
    writer.null();
    writer.finish()
}

fn singular_serial<D>(machine: &Machine<D>, node: NodeKey, kind: &str) -> Option<u64> {
    let mut keys = Vec::new();
    for event in machine.store.pending_events.values().flatten().copied() {
        let (key, owner, _, _, _, _) = event.identity();
        if owner == node && event.kind_name() == kind {
            keys.push(key.value());
        }
    }
    if keys.len() == 1 { keys.pop() } else { None }
}

fn stored_level<D>(machine: &Machine<D>, index: usize) -> LogicLevel {
    match machine.store.stored_levels.get(index).copied() {
        Some(level) => level,
        None => panic!("compiled state slot must resolve in the committed machine"),
    }
}

fn stored_edge<D>(machine: &Machine<D>, index: usize) -> EdgeObservation {
    match machine.store.edge_observations.get(index).copied() {
        Some(observation) => observation,
        None => panic!("compiled edge observation must resolve in the committed machine"),
    }
}

fn write_episodes<D>(writer: &mut Cbor, machine: &Machine<D>, provenance: &CauseDigestIndex) {
    let mut episodes = Vec::new();
    for (index, episode) in machine.store.active_episodes.values().enumerate() {
        let identity = episode.identity().as_bytes();
        let cause = episode_cause(provenance, index, episode);
        let mut record = Record::new();
        let began = episode.began_at().ticks();
        record.field("began_at", |writer| writer.uint(began));
        record.field("began_order", |writer| {
            writer.uint(episode.began_stamp().order())
        });
        record.field("cause", |writer| writer.bytes(&cause));
        let code = episode.condition().code().as_str();
        record.field("code", |writer| writer.text(code));
        let discriminator = u64::from(episode.condition().discriminator());
        record.field("discriminator", |writer| writer.uint(discriminator));
        let evidence = encode_episode_evidence(episode.current());
        record.field("evidence", |writer| writer.nested(&evidence));
        record.field("identity", |writer| writer.bytes(&identity));
        let changed = episode.last_material_change().ticks();
        record.field("last_material_change", |writer| writer.uint(changed));
        record.field("last_material_order", |writer| {
            writer.uint(episode.last_material_stamp().order())
        });
        let owner = encode_node_subject(episode.condition().owner());
        record.field("owner", |writer| writer.nested(&owner));
        let revision = episode_evidence_revision(episode.current());
        record.field("revision", |writer| writer.uint(revision));
        episodes.push((identity, record.finish()));
    }
    episodes.sort_by_key(|(identity, _)| *identity);
    writer.array_start(episodes.len());
    for (_, episode) in episodes {
        writer.nested(&episode);
    }
}

fn episode_cause<D>(
    provenance: &CauseDigestIndex,
    index: usize,
    episode: &ActiveDiagnosticEpisode<D>,
) -> [u8; 32] {
    let ordinal = episode.provenance().resolve_ordinal(episode.cause());
    let digest = provenance.episode_digest(index, ordinal);
    if !provenance.records().contains_key(&digest) {
        panic!("required provenance root must retain its canonical record");
    }
    digest
}

fn write_provenance<D>(writer: &mut Cbor, machine: &Machine<D>, provenance: &CauseDigestIndex) {
    let mut record = Record::new();
    record.field("current_roots", |writer| {
        let rows = crate::causal_roots::bindings(machine, true);
        writer.array_start(rows.len());
        for (subject, role, cause) in rows {
            let digest = cause_digest(machine, provenance, cause);
            let mut row = Record::new();
            row.field("cause", |writer| writer.bytes(&digest));
            row.field("role", |writer| writer.text(role));
            row.field("subject", |writer| writer.nested(&subject));
            writer.nested(&row.finish());
        }
    });
    let episodes = episode_roots(machine, provenance);
    record.field("episodes", |writer| write_digests(writer, &episodes));
    let inputs = cause_set(machine, provenance, &machine.store.input_causes);
    record.field("external_inputs", |writer| write_digests(writer, &inputs));
    let baselines = cause_set(machine, provenance, &machine.store.output_causes);
    record.field("output_baselines", |writer| {
        write_digests(writer, &baselines)
    });
    let pending = pending_roots(machine, provenance);
    record.field("pending_events", |writer| write_digests(writer, &pending));
    // SPEC: docs/specs/contracts/machine-snapshot-artifact.yaml "self-contained-provenance"
    // Each persisted record is the existing canonical CauseDigest record.
    record.field("records", |writer| {
        writer.array_start(provenance.records().len());
        for payload in provenance.records().values() {
            #[cfg(test)]
            crate::projection_work::update(|work| {
                work.records_emitted += 1;
                work.bytes_emitted += payload.len();
            });
            writer.nested(payload);
        }
    });
    let state = state_roots(machine, provenance);
    record.field("state", |writer| write_digests(writer, &state));
    writer.nested(&record.finish());
}

fn write_digests(writer: &mut Cbor, digests: &BTreeSet<[u8; 32]>) {
    writer.array_start(digests.len());
    for digest in digests {
        writer.bytes(digest);
    }
}

fn episode_roots<D>(machine: &Machine<D>, provenance: &CauseDigestIndex) -> BTreeSet<[u8; 32]> {
    machine
        .store
        .active_episodes
        .values()
        .enumerate()
        .map(|(index, episode)| episode_cause(provenance, index, episode))
        .collect()
}

fn pending_roots<D>(machine: &Machine<D>, provenance: &CauseDigestIndex) -> BTreeSet<[u8; 32]> {
    machine
        .store
        .pending_events
        .values()
        .flatten()
        .map(|event| cause_digest(machine, provenance, event.identity().5))
        .collect()
}

fn state_roots<D>(machine: &Machine<D>, provenance: &CauseDigestIndex) -> BTreeSet<[u8; 32]> {
    let mut roots = BTreeSet::new();
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.edge_observation_causes,
    );
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.toggle_inversion_causes,
    );
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.establishment_causes,
    );
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.transport_transition_causes,
    );
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.inertial_cancellation_causes,
    );
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.periodic_anchor_causes,
    );
    extend_causes(
        &mut roots,
        machine,
        provenance,
        &machine.store.periodic_cancellation_causes,
    );
    roots
}

fn cause_set<D, K>(
    machine: &Machine<D>,
    provenance: &CauseDigestIndex,
    causes: &BTreeMap<K, crate::CauseRef>,
) -> BTreeSet<[u8; 32]> {
    let mut roots = BTreeSet::new();
    extend_causes(&mut roots, machine, provenance, causes);
    roots
}

fn extend_causes<D, K>(
    roots: &mut BTreeSet<[u8; 32]>,
    machine: &Machine<D>,
    provenance: &CauseDigestIndex,
    causes: &BTreeMap<K, crate::CauseRef>,
) {
    for cause in causes.values().copied() {
        roots.insert(cause_digest(machine, provenance, cause));
    }
}

fn cause_digest<D>(
    machine: &Machine<D>,
    provenance: &CauseDigestIndex,
    cause: crate::CauseRef,
) -> [u8; 32] {
    let view = match &machine.store.provenance {
        Some(view) => view,
        None => panic!("committed cause must belong to the machine provenance view"),
    };
    let digest = provenance.machine_digest(view.resolve_ordinal(cause));
    if !provenance.records().contains_key(&digest) {
        panic!("required provenance root must retain its canonical record");
    }
    digest
}

pub(crate) fn snapshot_digest_bytes(time_domain: TimeDomainId, payload: &[u8]) -> [u8; 32] {
    let bare = envelope(time_domain, payload, None);
    *blake3::hash(&domain_separated(SNAPSHOT_DIGEST_DOMAIN, 2, &bare)).as_bytes()
}

fn envelope(time_domain: TimeDomainId, payload: &[u8], integrity: Option<[u8; 32]>) -> Vec<u8> {
    let mut record = Record::new();
    record.field("artifact_kind", |writer| writer.text("machine_snapshot"));
    version(&mut record, "artifact_schema_version");
    version(&mut record, "canonical_encoding_version");
    version(&mut record, "core_semantics_version");
    version(&mut record, "diagnostic_schema_version");
    version(&mut record, "digest_suite_version");
    version(&mut record, "envelope_schema_version");
    if let Some(digest) = integrity {
        record.field("integrity_digest", |writer| writer.bytes(&digest));
    }
    version(&mut record, "node_semantics_version");
    version(&mut record, "patch_semantics_version");
    record.field("payload", |writer| writer.nested(payload));
    version(&mut record, "provenance_semantics_version");
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

#[cfg(test)]
mod tests {
    use super::snapshot_with_label;
    use crate::key::{ExternalInputKey, ExternalOutputKey, ModuleInstanceKey, NetworkKey, NodeKey};
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
    use crate::time::{NonZeroSpan, Time};
    use crate::{
        ConflictPolicy, EdgeConfig, EdgeInitialization, EncodeFailure, FirstEmissionPolicy,
        InertialDelayConfig, LevelSetResetConfig, Machine, ModuleBuilder, NetworkBuilder,
        PeriodicConfig, PersistenceContext, PulseDelayConfig, ReenablePhasePolicy, RuntimePolicy,
        SampleHoldConfig, TimeDomainId, ToggleConfig, Transaction, TransportDelayConfig,
        encode_snapshot,
    };
    use std::fs;
    use std::path::PathBuf;

    const PREFIX: [u8; 8] = [0x4d, 0x53, 0x49, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    const SNAPSHOT_DOMAIN: &str = "mossignal/snapshot_digest/v2";
    const PROVENANCE_DOMAIN: &str = "mossignal/provenance_record/v3";
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
        "provenance",
        "runtime_policy_id",
        "semantic_versions",
        "temporal_state_table",
        "time_domain_id",
        "topology_revision",
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

    #[derive(Clone, Copy)]
    struct Expect {
        ready: bool,
        label: Option<&'static str>,
        families: bool,
    }

    fn policy() -> RuntimePolicy {
        match RuntimePolicy::builder()
            .max_internal_reactions(100)
            .max_evaluated_operations(10_000)
            .max_pending_events(100)
            .max_events_created_per_transaction(100)
            .max_required_provenance_growth(10_000)
            .build()
        {
            Ok(policy) => policy,
            Err(failure) => panic!("complete policy must build: {failure}"),
        }
    }

    fn span(ticks: u64) -> NonZeroSpan<()> {
        match NonZeroSpan::from_ticks(ticks) {
            Ok(span) => span,
            Err(failure) => panic!("fixture span must be positive: {failure}"),
        }
    }

    fn compile(builder: NetworkBuilder<()>) -> crate::CompiledNetwork<()> {
        match builder.finish().require_artifact() {
            Ok(network) => match network.compile().require_artifact() {
                Ok(compiled) => compiled,
                Err(failure) => panic!("fixture must compile: {failure:?}"),
            },
            Err(failure) => panic!("fixture must validate: {failure:?}"),
        }
    }

    fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(failure) => panic!("fixture authoring must succeed: {failure:?}"),
        }
    }

    fn uninitialized_toggle() -> Machine<()> {
        let meta = DiagnosticMeta::default();
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let input =
            must(builder.add_pulse_input(ExternalInputKey::<Pulse>::from_u128(6), meta.clone()));
        let toggle = must(builder.add_toggle(
            NodeKey::from_u128(3),
            input,
            ToggleConfig::new(LogicLevel::Low),
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_level_output(ExternalOutputKey::<Level>::from_u128(8), toggle, meta));
        compile(builder).spawn(policy())
    }

    fn families() -> Machine<()> {
        let meta = DiagnosticMeta::default();
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let level =
            must(builder.add_level_input(ExternalInputKey::<Level>::from_u128(10), meta.clone()));
        let pulse_a =
            must(builder.add_pulse_input(ExternalInputKey::<Pulse>::from_u128(11), meta.clone()));
        let pulse_b =
            must(builder.add_pulse_input(ExternalInputKey::<Pulse>::from_u128(12), meta.clone()));
        let high =
            must(builder.add_constant(NodeKey::from_u128(13), LogicLevel::High, meta.clone()))
                .into_outputs();
        must(builder.add_rising_edge(
            NodeKey::from_u128(20),
            level,
            EdgeConfig::new(EdgeInitialization::Baseline),
            meta.clone(),
        ));
        let toggle = must(builder.add_toggle(
            NodeKey::from_u128(21),
            pulse_a,
            ToggleConfig::new(LogicLevel::Low),
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_level_set_reset_latch(
            NodeKey::from_u128(22),
            high,
            high,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta.clone(),
        ));
        must(builder.add_level_set_reset_latch(
            NodeKey::from_u128(23),
            high,
            high,
            LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
            meta.clone(),
        ));
        must(builder.add_sample_hold(
            NodeKey::from_u128(24),
            level,
            pulse_a,
            SampleHoldConfig::new(LogicLevel::Low),
            meta.clone(),
        ));
        must(builder.add_pulse_delay(
            NodeKey::from_u128(25),
            pulse_a,
            PulseDelayConfig::new(span(5)),
            meta.clone(),
        ));
        must(builder.add_pulse_delay(
            NodeKey::from_u128(26),
            pulse_b,
            PulseDelayConfig::new(span(5)),
            meta.clone(),
        ));
        let transport = must(builder.add_transport_delay(
            NodeKey::from_u128(27),
            level,
            TransportDelayConfig::new(span(4), LogicLevel::Low),
            meta.clone(),
        ))
        .into_outputs();
        must(builder.add_inertial_delay(
            NodeKey::from_u128(28),
            level,
            InertialDelayConfig::new(span(6), LogicLevel::Low),
            meta.clone(),
        ));
        must(builder.add_periodic(
            NodeKey::from_u128(29),
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
        let snapshot = match compiled
            .input_snapshot()
            .set(ExternalInputKey::<Level>::from_u128(10), LogicLevel::High)
            .and_then(|builder| {
                builder.pulse(ExternalInputKey::<Pulse>::from_u128(11), PulseCount::ONE)
            })
            .and_then(|builder| {
                builder.pulse(ExternalInputKey::<Pulse>::from_u128(12), PulseCount::ONE)
            })
            .and_then(crate::InputSnapshotBuilder::finish)
        {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("fixture snapshot must bind: {failure}"),
        };
        match machine.apply(Transaction::initialize(
            Time::from_ticks(1),
            machine.revision(),
            snapshot,
        )) {
            Ok(_) => machine,
            Err(failure) => panic!("fixture initialization must commit: {failure}"),
        }
    }

    fn golden_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name)
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

    fn assert_golden(name: &str, bytes: &[u8]) {
        let path = golden_path(name);
        let actual = hex(bytes);
        let expected = fs::read_to_string(&path)
            .unwrap_or_else(|failure| panic!("missing golden {}: {failure}", path.display()));
        assert_eq!(actual, expected.trim(), "golden {}", path.display());
    }

    fn encode<D>(machine: &Machine<D>) -> (crate::MachineSnapshot<D>, Vec<u8>) {
        let snapshot = machine.snapshot();
        let context = PersistenceContext::new(machine.compiled().time_domain_id());
        let bytes = match encode_snapshot(&context, &snapshot) {
            Ok(bytes) => bytes,
            Err(failure) => panic!("matching context must encode: {failure}"),
        };
        (snapshot, bytes.as_bytes().to_vec())
    }

    #[test]
    fn uninitialized_snapshot_matches_the_golden_and_required_fields() {
        let machine = uninitialized_toggle();
        let execution = machine.execution_state_digest();
        let observable = machine.observable_state_digest();
        let (snapshot, bytes) = encode(&machine);
        assert_eq!(machine.execution_state_digest(), execution);
        assert_eq!(machine.observable_state_digest(), observable);
        assert_eq!(snapshot.execution_state_digest(), execution);
        assert_eq!(snapshot.observable_state_digest(), observable);
        assert_eq!(snapshot.revision(), machine.revision());
        assert_eq!(snapshot.fingerprint(), machine.fingerprint());
        assert_eq!(snapshot.runtime_policy_id(), machine.policy.id());
        assert!(matches!(
            snapshot.status(),
            crate::MachineStatus::AwaitingInitialization
        ));
        inspect(
            &bytes,
            &snapshot,
            Expect {
                ready: false,
                label: None,
                families: false,
            },
        );
        assert!(
            bytes
                .windows(key_cbor(1).len())
                .any(|window| window == key_cbor(1))
        );
        assert!(
            bytes
                .windows(key_cbor(2).len())
                .any(|window| window == key_cbor(2))
        );
        assert_golden("machine_snapshot_uninitialized.hex", &bytes);
        assert_golden(
            "machine_snapshot_uninitialized_digest.hex",
            &snapshot.snapshot_digest().as_bytes(),
        );
    }

    #[test]
    fn ready_snapshot_covers_state_families_pending_work_and_an_episode() {
        let machine = families();
        assert!(!machine.store.active_episodes.is_empty());
        assert!(machine.store.pending_events.values().flatten().count() >= 4);
        let (snapshot, bytes) = encode(&machine);
        assert_eq!(
            snapshot.execution_state_digest(),
            machine.execution_state_digest()
        );
        assert_eq!(
            snapshot.observable_state_digest(),
            machine.observable_state_digest()
        );
        inspect(
            &bytes,
            &snapshot,
            Expect {
                ready: true,
                label: None,
                families: true,
            },
        );
        assert_golden("machine_snapshot_families.hex", &bytes);
        assert_golden(
            "machine_snapshot_families_digest.hex",
            &snapshot.snapshot_digest().as_bytes(),
        );
    }

    #[test]
    fn private_order_does_not_change_canonical_bytes() {
        let mut machine = families();
        let (_, original) = encode(&machine);
        let before_keys = machine.pending_keys_in_storage_order();
        machine.reverse_pending_batches_for_test();
        assert_ne!(machine.pending_keys_in_storage_order(), before_keys);
        let ordinals = machine.supporter_ordinals_for_test();
        assert!(ordinals.len() >= 2);
        machine.reverse_provenance_supporters_for_test();
        assert_ne!(machine.supporter_ordinals_for_test(), ordinals);
        assert!(machine.store.active_episodes.len() >= 2);
        machine.rebuild_episodes_reversed_for_test();
        let (_, reordered) = encode(&machine);
        assert_eq!(reordered, original);
    }

    #[test]
    fn metadata_changes_snapshot_digest_only() {
        let machine = uninitialized_toggle();
        let plain = machine.snapshot();
        let labeled = snapshot_with_label(&machine, "inspector");
        assert_eq!(
            plain.execution_state_digest(),
            labeled.execution_state_digest()
        );
        assert_eq!(
            plain.observable_state_digest(),
            labeled.observable_state_digest()
        );
        assert_eq!(
            plain.execution_state_digest(),
            machine.execution_state_digest()
        );
        assert_ne!(plain.snapshot_digest(), labeled.snapshot_digest());
        let context = PersistenceContext::new(machine.compiled().time_domain_id());
        let labeled_bytes = match encode_snapshot(&context, &labeled) {
            Ok(bytes) => bytes.as_bytes().to_vec(),
            Err(failure) => panic!("matching context must encode: {failure}"),
        };
        inspect(
            &labeled_bytes,
            &labeled,
            Expect {
                ready: false,
                label: Some("inspector"),
                families: false,
            },
        );
        assert!(cbor_text(&labeled_bytes, "inspector"));
        let (_, plain_bytes) = encode(&machine);
        assert!(!cbor_text(&plain_bytes, "persistence_metadata"));
    }

    #[test]
    fn context_time_domain_mismatch_emits_no_artifact() {
        let machine = uninitialized_toggle();
        let snapshot = machine.snapshot();
        let context = PersistenceContext::new(TimeDomainId::from_u128(99));
        let failure = match encode_snapshot(&context, &snapshot) {
            Ok(_) => panic!("mismatched time domain must not emit bytes"),
            Err(failure) => failure,
        };
        assert_eq!(failure, EncodeFailure::TimeDomainMismatch);
        assert_eq!(
            machine.execution_state_digest(),
            snapshot.execution_state_digest()
        );
    }

    #[test]
    fn nested_instance_path_order_is_semantic() {
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
        let machine = compile(builder).spawn(policy());
        let (_, bytes) = encode(&machine);
        let mut ordered = vec![0x82];
        ordered.extend(key_cbor(0x61));
        ordered.extend(key_cbor(0x50));
        assert!(
            bytes.windows(ordered.len()).any(|window| window == ordered),
            "outer instance must precede the nested instance"
        );
        let mut swapped = vec![0x82];
        swapped.extend(key_cbor(0x50));
        swapped.extend(key_cbor(0x61));
        assert!(
            bytes.windows(swapped.len()).all(|window| window != swapped),
            "instance path order must stay outermost to innermost"
        );
    }

    fn key_cbor(value: u128) -> Vec<u8> {
        let mut bytes = vec![0x50];
        bytes.extend(value.to_be_bytes());
        bytes
    }

    fn cbor_text(bytes: &[u8], text: &str) -> bool {
        let encoded = text_encoding(text);
        bytes.windows(encoded.len()).any(|window| window == encoded)
    }

    fn text_encoding(text: &str) -> Vec<u8> {
        let mut encoded = Vec::new();
        let len = text.len() as u64;
        if len <= 23 {
            encoded.push((3 << 5) | len as u8);
        } else if len <= u64::from(u8::MAX) {
            encoded.extend([0x78, len as u8]);
        } else {
            panic!("fixture text must fit in one length byte");
        }
        encoded.extend(text.as_bytes());
        encoded
    }

    fn inspect(bytes: &[u8], snapshot: &crate::MachineSnapshot<()>, expect: Expect) {
        assert_eq!(&bytes[..PREFIX.len()], &PREFIX);
        let mut cursor = Cursor {
            bytes: &bytes[PREFIX.len()..],
            index: 0,
        };
        let value = cursor.value();
        assert!(cursor.finished());
        assert_records_canonical(&value);
        let (kind, envelope) = variant(&value);
        assert_eq!(kind, "mossignal_artifact");
        let fields = record(envelope);
        assert_eq!(field_names(&fields), ENVELOPE_FIELDS);
        for name in VERSION_FIELDS {
            assert_eq!(
                uint(field(&fields, name)),
                if matches!(
                    *name,
                    "artifact_schema_version" | "provenance_semantics_version"
                ) {
                    3
                } else {
                    2
                },
                "{name}"
            );
        }
        assert_eq!(text(field(&fields, "artifact_kind")), "machine_snapshot");
        let integrity = byte_string(field(&fields, "integrity_digest"));
        assert_eq!(integrity, snapshot.snapshot_digest().as_bytes());
        let digest = snapshot_digest_of(envelope);
        assert_eq!(digest.as_slice(), integrity.as_slice());
        let payload = record(field(&fields, "payload"));
        let mut payload_names = field_names(&payload);
        if expect.label.is_some() {
            payload_names.retain(|name| *name != "persistence_metadata");
        }
        assert_eq!(payload_names, PAYLOAD_FIELDS);
        assert_eq!(
            byte_string(field(&payload, "execution_state_digest")).as_slice(),
            snapshot.execution_state_digest().as_bytes()
        );
        assert_eq!(
            byte_string(field(&payload, "observable_state_digest")).as_slice(),
            snapshot.observable_state_digest().as_bytes()
        );
        assert_eq!(
            byte_string(field(&payload, "network_fingerprint")).as_slice(),
            snapshot.fingerprint().as_bytes()
        );
        assert_eq!(
            byte_string(field(&payload, "runtime_policy_id")).as_slice(),
            snapshot.runtime_policy_id().as_bytes()
        );
        assert_eq!(byte_string(field(&payload, "network_key")).len(), 16);
        assert_eq!(byte_string(field(&payload, "time_domain_id")).len(), 16);
        assert_eq!(
            uint(field(&payload, "topology_revision")),
            snapshot.revision().value()
        );
        let versions = record(field(&payload, "semantic_versions"));
        for name in [
            "core_semantics_version",
            "diagnostic_schema_version",
            "node_semantics_version",
            "patch_semantics_version",
            "provenance_semantics_version",
        ] {
            assert_eq!(
                uint(field(&versions, name)),
                if name == "provenance_semantics_version" {
                    3
                } else {
                    2
                },
                "{name}"
            );
        }
        let (lifecycle_name, lifecycle_body) = variant(field(&payload, "lifecycle"));
        let provenance = record(field(&payload, "provenance"));
        assert_eq!(
            field_names(&provenance),
            [
                "current_roots",
                "episodes",
                "external_inputs",
                "output_baselines",
                "pending_events",
                "records",
                "state",
            ]
        );
        let records = array(field(&provenance, "records"));
        let mut record_digests = Vec::new();
        for item in records {
            record_digests.push(provenance_digest(item));
        }
        let mut sorted = record_digests.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(record_digests, sorted);
        for root_name in [
            "episodes",
            "external_inputs",
            "output_baselines",
            "pending_events",
            "state",
        ] {
            for root in array(field(&provenance, root_name)) {
                let root = byte_string(root);
                assert!(
                    record_digests
                        .iter()
                        .any(|digest| digest.as_slice() == root),
                    "provenance root must name a persisted record"
                );
            }
        }
        if expect.ready {
            assert_eq!(lifecycle_name, "ready");
            let ready = record(lifecycle_body);
            assert_eq!(
                field_names(&ready),
                [
                    "explanation_boundary",
                    "external_levels",
                    "output_baselines",
                    "pending_events",
                    "reaction_order",
                    "settled_levels",
                    "time",
                ]
            );
            let (boundary, boundary_body) = variant(field(&ready, "explanation_boundary"));
            assert_eq!(boundary, "complete_from_initialization");
            assert!(matches!(boundary_body, Value::Null));
            let time = uint(field(&ready, "time"));
            let mut serials = Vec::new();
            for event in array(field(&ready, "pending_events")) {
                let event = record(event);
                assert_eq!(
                    field_names(&event),
                    [
                        "cause",
                        "deadline",
                        "key",
                        "kind",
                        "origin_revision",
                        "origin_time",
                        "owner",
                        "stimulus_order",
                        "stimulus_time"
                    ]
                );
                assert!(uint(field(&event, "deadline")) > time);
                serials.push(uint(field(&event, "key")));
                let (kind_name, _) = variant(field(&event, "kind"));
                assert!(matches!(
                    kind_name,
                    "pulse_delay_group"
                        | "transport_transition"
                        | "inertial_maturation"
                        | "periodic_boundary"
                ));
            }
            let next = uint(field(&payload, "next_pending_event_serial"));
            assert!(serials.into_iter().all(|serial| serial < next));
            assert!(array(field(&ready, "settled_levels")).iter().any(|fact| {
                let fact = record(fact);
                let (subject, _) = variant(field(&fact, "subject"));
                subject == "external_input"
            }));
        } else {
            assert_eq!(lifecycle_name, "awaiting_initialization");
            assert!(matches!(lifecycle_body, Value::Null));
            assert!(array(field(&payload, "active_diagnostic_episodes")).is_empty());
            assert!(records.is_empty());
        }
        if expect.families {
            for name in [
                "edge_observation",
                "stored_level",
                "pending_pulse_group",
                "remembered_input_output",
                "pending_transport_transition",
                "pending_inertial_candidate",
                "periodic_anchor_previous_enable",
                "pending_periodic_boundary",
                "pulse_delay_group",
                "transport_transition",
                "inertial_maturation",
                "periodic_boundary",
                "complete_from_initialization",
                "runtime.level_latch_conflict_retained",
                "external_input",
            ] {
                assert!(cbor_text(bytes, name), "artifact must contain {name}");
            }
            assert!(!array(field(&payload, "active_diagnostic_episodes")).is_empty());
        }
        if let Some(label) = expect.label {
            let meta = record(field(&payload, "persistence_metadata"));
            assert_eq!(text(field(&meta, "label")), label);
        }
        for entry in array(field(&payload, "node_state_table"))
            .iter()
            .chain(array(field(&payload, "temporal_state_table")))
        {
            let entry = record(entry);
            assert_eq!(
                field_names(&entry),
                ["node_kind", "owner", "schema", "value"]
            );
        }
    }

    fn snapshot_digest_of(envelope: &Value) -> Vec<u8> {
        let fields = record(envelope)
            .into_iter()
            .filter(|(name, _)| name != "integrity_digest")
            .collect::<Vec<_>>();
        let bare = encode_record(&fields);
        blake3::hash(&domain_record(SNAPSHOT_DOMAIN, 2, &bare))
            .as_bytes()
            .to_vec()
    }

    fn provenance_digest(record: &Value) -> Vec<u8> {
        let encoded = encode_value(record);
        blake3::hash(&domain_record(PROVENANCE_DOMAIN, 3, &encoded))
            .as_bytes()
            .to_vec()
    }

    fn domain_record(domain: &str, version: u64, payload: &[u8]) -> Vec<u8> {
        let mut writer = Writer::default();
        writer.array(3);
        writer.array(2);
        writer.text("domain");
        writer.text(domain);
        writer.array(2);
        writer.text("payload");
        writer.splice(payload);
        writer.array(2);
        writer.text("version");
        writer.uint(version);
        writer.0
    }

    fn encode_record(fields: &[(String, Value)]) -> Vec<u8> {
        let mut writer = Writer::default();
        writer.array(fields.len());
        for (name, value) in fields {
            writer.array(2);
            writer.text(name);
            writer.value(value);
        }
        writer.0
    }

    fn encode_value(value: &Value) -> Vec<u8> {
        let mut writer = Writer::default();
        writer.value(value);
        writer.0
    }

    #[derive(Clone, Debug)]
    enum Value {
        Uint(u64),
        Bytes(Vec<u8>),
        Text(String),
        Array(Vec<Value>),
        Bool(bool),
        Null,
    }

    struct Cursor<'a> {
        bytes: &'a [u8],
        index: usize,
    }

    impl<'a> Cursor<'a> {
        fn finished(&self) -> bool {
            self.index == self.bytes.len()
        }

        fn value(&mut self) -> Value {
            let initial = self.byte();
            let major = initial >> 5;
            let info = initial & 0x1f;
            if info == 31 {
                panic!("canonical artifact must not use indefinite CBOR");
            }
            let argument = self.argument(info);
            match major {
                0 => Value::Uint(argument),
                2 => Value::Bytes(self.take(argument)),
                3 => {
                    let bytes = self.take(argument);
                    match String::from_utf8(bytes) {
                        Ok(text) => Value::Text(text),
                        Err(failure) => panic!("canonical text must be UTF-8: {failure}"),
                    }
                }
                4 => {
                    let mut items = Vec::new();
                    for _ in 0..argument {
                        items.push(self.value());
                    }
                    Value::Array(items)
                }
                7 => match argument {
                    20 => Value::Bool(false),
                    21 => Value::Bool(true),
                    22 => Value::Null,
                    _ => panic!("canonical artifact must not use unsupported simple values"),
                },
                _ => panic!("canonical artifact must not use major type {major}"),
            }
        }

        fn argument(&mut self, info: u8) -> u64 {
            match info {
                0..=23 => u64::from(info),
                24 => u64::from(self.byte()),
                25 => u64::from(u16::from_be_bytes(self.fixed())),
                26 => u64::from(u32::from_be_bytes(self.fixed())),
                27 => u64::from_be_bytes(self.fixed()),
                _ => panic!("canonical length must be definite"),
            }
        }

        fn byte(&mut self) -> u8 {
            let byte = match self.bytes.get(self.index).copied() {
                Some(byte) => byte,
                None => panic!("canonical value must be complete"),
            };
            self.index += 1;
            byte
        }

        fn fixed<const N: usize>(&mut self) -> [u8; N] {
            let mut bytes = [0; N];
            for byte in &mut bytes {
                *byte = self.byte();
            }
            bytes
        }

        fn take(&mut self, len: u64) -> Vec<u8> {
            let len = usize::try_from(len).unwrap_or_else(|_| panic!("canonical length must fit"));
            let end = self
                .index
                .checked_add(len)
                .unwrap_or_else(|| panic!("canonical value must be complete"));
            let bytes = match self.bytes.get(self.index..end) {
                Some(bytes) => bytes.to_vec(),
                None => panic!("canonical value must be complete"),
            };
            self.index = end;
            bytes
        }
    }

    fn assert_records_canonical(value: &Value) {
        if let Some(fields) = as_record(value) {
            let names = field_names(&fields);
            let mut sorted = names.clone();
            sorted.sort_unstable();
            assert_eq!(names, sorted, "record fields must be in canonical order");
            let mut unique = sorted.clone();
            unique.dedup();
            assert_eq!(sorted, unique, "record field names must be unique");
            for (_, child) in fields {
                assert_records_canonical(&child);
            }
        } else if let Value::Array(items) = value {
            for item in items {
                assert_records_canonical(item);
            }
        }
    }

    fn as_record(value: &Value) -> Option<Vec<(String, Value)>> {
        let Value::Array(items) = value else {
            return None;
        };
        if items.is_empty() {
            return None;
        }
        let mut fields = Vec::new();
        for item in items {
            let Value::Array(pair) = item else {
                return None;
            };
            if pair.len() != 2 {
                return None;
            }
            let Value::Text(name) = &pair[0] else {
                return None;
            };
            fields.push((name.clone(), pair[1].clone()));
        }
        Some(fields)
    }

    fn record(value: &Value) -> Vec<(String, Value)> {
        match as_record(value) {
            Some(fields) => fields,
            None => panic!("expected a canonical record"),
        }
    }

    fn field<'a>(fields: &'a [(String, Value)], name: &str) -> &'a Value {
        match fields.iter().find(|(field, _)| field == name) {
            Some((_, value)) => value,
            None => panic!("missing field {name}"),
        }
    }

    fn field_names(fields: &[(String, Value)]) -> Vec<&str> {
        fields.iter().map(|(name, _)| name.as_str()).collect()
    }

    fn variant(value: &Value) -> (&str, &Value) {
        let Value::Array(items) = value else {
            panic!("expected a canonical variant");
        };
        if items.len() != 2 {
            panic!("expected a two-element variant");
        }
        let Value::Text(name) = &items[0] else {
            panic!("expected a variant name");
        };
        (name, &items[1])
    }

    fn array(value: &Value) -> &[Value] {
        match value {
            Value::Array(items) => items,
            _ => panic!("expected an array"),
        }
    }

    fn text(value: &Value) -> &str {
        match value {
            Value::Text(text) => text,
            _ => panic!("expected text"),
        }
    }

    fn uint(value: &Value) -> u64 {
        match value {
            Value::Uint(value) => *value,
            _ => panic!("expected an unsigned integer"),
        }
    }

    fn byte_string(value: &Value) -> Vec<u8> {
        match value {
            Value::Bytes(bytes) => bytes.clone(),
            _ => panic!("expected a byte string"),
        }
    }

    #[derive(Default)]
    struct Writer(Vec<u8>);

    impl Writer {
        fn value(&mut self, value: &Value) {
            match value {
                Value::Uint(value) => self.uint(*value),
                Value::Bytes(bytes) => self.bytes(bytes),
                Value::Text(text) => self.text(text),
                Value::Array(items) => {
                    self.array(items.len());
                    for item in items {
                        self.value(item);
                    }
                }
                Value::Bool(value) => self.0.push(if *value { 0xf5 } else { 0xf4 }),
                Value::Null => self.0.push(0xf6),
            }
        }

        fn uint(&mut self, value: u64) {
            self.major(0, value);
        }

        fn bytes(&mut self, value: &[u8]) {
            self.major(2, value.len() as u64);
            self.0.extend(value);
        }

        fn text(&mut self, value: &str) {
            self.major(3, value.len() as u64);
            self.0.extend(value.as_bytes());
        }

        fn array(&mut self, len: usize) {
            self.major(4, len as u64);
        }

        fn splice(&mut self, bytes: &[u8]) {
            self.0.extend(bytes);
        }

        fn major(&mut self, major: u8, value: u64) {
            let initial = major << 5;
            if value <= 23 {
                self.0.push(initial | value as u8);
            } else if u8::try_from(value).is_ok() {
                self.0.extend([initial | 24, value as u8]);
            } else if u16::try_from(value).is_ok() {
                self.0.push(initial | 25);
                self.0.extend_from_slice(&(value as u16).to_be_bytes());
            } else if u32::try_from(value).is_ok() {
                self.0.push(initial | 26);
                self.0.extend_from_slice(&(value as u32).to_be_bytes());
            } else {
                self.0.push(initial | 27);
                self.0.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
}
