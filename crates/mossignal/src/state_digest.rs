//! Canonical execution-state and observable-state projections.
//!
//! Projection version 3 is the record written here and locked by the golden
//! digest-input vectors. `standard_history` stays out of both digests: it is
//! last-reaction inspection cache, and current explanation is the execution
//! projection plus the required provenance closure.

use crate::authored::{ConflictPolicy, EdgeObservation, FirstEmissionPolicy, ReenablePhasePolicy};
use crate::compile::{CompiledNetwork, DigestStateFamily, SettledEndpoint, StableOwner};
use crate::diagnostics::{ConflictControls, Problem, ProblemEvidence};
use crate::identity::{
    Cbor, EXECUTION_STATE_DOMAIN, ExecutionStateDigest, OBSERVABLE_STATE_DOMAIN,
    ObservableStateDigest, PROVENANCE_RECORD_DOMAIN, domain_separated, logic_level,
};
use crate::key::ModuleInstanceKey;
use crate::machine::{Machine, PendingEvent};
use crate::module::{NodeSubject, PulsePortSubject, QualifiedNodeRef};
use crate::signal::LogicLevel;
use crate::transaction::{
    CauseRef, ProvenanceRecord, ProvenanceSubjectKind, ProvenanceView, PulseContribution,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(crate) fn execution_state_digest<D>(machine: &Machine<D>) -> ExecutionStateDigest {
    let input = execution_digest_input(machine, 3, 3);
    ExecutionStateDigest::from_digest(*blake3::hash(&input).as_bytes())
}

pub(crate) fn observable_state_digest<D>(machine: &Machine<D>) -> ObservableStateDigest {
    let input = observable_digest_input(machine, 3, 3);
    ObservableStateDigest::from_digest(*blake3::hash(&input).as_bytes())
}

pub(crate) fn declared_state_checkpoint<D>(machine: &Machine<D>) -> Vec<u8> {
    let mut fact = Record::new();
    fact.field("declared_state", |writer| write_state(writer, machine));
    fact.finish()
}

pub(crate) fn execution_digest_input<D>(
    machine: &Machine<D>,
    projection_version: u64,
    domain_version: u64,
) -> Vec<u8> {
    let payload = projection_payload(machine, projection_version, false);
    domain_separated(EXECUTION_STATE_DOMAIN, domain_version, &payload)
}

pub(crate) fn observable_digest_input<D>(
    machine: &Machine<D>,
    projection_version: u64,
    domain_version: u64,
) -> Vec<u8> {
    let payload = projection_payload(machine, projection_version, true);
    domain_separated(OBSERVABLE_STATE_DOMAIN, domain_version, &payload)
}

fn projection_payload<D>(
    machine: &Machine<D>,
    projection_version: u64,
    observable: bool,
) -> Vec<u8> {
    let context = ProjectionContext::build(machine);
    let ready = machine.is_initialized();
    let mut record = Record::new();
    record.field("active_episodes", |writer| {
        write_episodes(writer, machine, &context);
    });
    if observable && ready {
        record.field("explanation_boundary", |writer| {
            writer.variant_null("complete_from_initialization");
        });
    }
    if ready {
        record.field("current_causes", |writer| {
            let rows = crate::causal_roots::bindings(machine, false);
            writer.array_start(rows.len());
            for (subject, role, cause) in rows {
                let digest = context.machine_cause(machine, cause);
                let mut row = Record::new();
                row.field("cause", |writer| writer.bytes(&digest));
                row.field("role", |writer| writer.text(role));
                row.field("subject", |writer| writer.nested(&subject));
                writer.nested(&row.finish());
            }
        });
    }
    record.field("lifecycle", |writer| write_lifecycle(writer, machine));
    record.field("network_fingerprint", |writer| {
        writer.bytes(&machine.fingerprint().as_bytes());
    });
    record.field("next_pending_event_serial", |writer| {
        writer.uint(machine.store.next_pending_event_serial);
    });
    if observable && ready {
        record.field("output_baselines", |writer| {
            write_baselines(writer, machine, &context);
        });
    }
    record.field("pending_events", |writer| {
        write_pending_events(writer, machine, &context);
    });
    record.field("projection_version", |writer| {
        writer.uint(projection_version)
    });
    if observable && ready {
        record.field("provenance", |writer| write_provenance(writer, &context));
    }
    record.field("revision", |writer| {
        writer.uint(machine.revision().value());
    });
    if observable && ready {
        record.field("settled_levels", |writer| {
            write_settled_levels(writer, machine)
        });
    }
    record.field("state", |writer| write_state(writer, machine));
    record.field("time_domain_id", |writer| {
        writer.bytes(&machine.compiled.time_domain_id().to_be_bytes());
    });
    record.finish()
}

struct ProjectionContext {
    table: ContentTable,
    machine_digests: Vec<[u8; 32]>,
    machine_reachable: BTreeSet<[u8; 32]>,
    episode_digests: Vec<Vec<[u8; 32]>>,
    episode_reachable: Vec<BTreeSet<[u8; 32]>>,
}

impl ProjectionContext {
    fn build<D>(machine: &Machine<D>) -> Self {
        let mut table = ContentTable::default();
        let (machine_digests, machine_reachable) = match &machine.store.provenance {
            Some(view) => {
                let digests = index_view(&machine.compiled, view, &mut table);
                let reachable = reachable_digests(view, &digests, &machine_roots(machine));
                (digests, reachable)
            }
            None => (Vec::new(), BTreeSet::new()),
        };
        let mut episode_digests = Vec::new();
        let mut episode_reachable = Vec::new();
        for episode in machine.store.active_episodes.values() {
            let view = episode.provenance();
            let digests = index_view(&machine.compiled, view, &mut table);
            let reachable = reachable_digests(view, &digests, &[episode.cause()]);
            episode_digests.push(digests);
            episode_reachable.push(reachable);
        }
        Self {
            table,
            machine_digests,
            machine_reachable,
            episode_digests,
            episode_reachable,
        }
    }

    fn machine_cause<D>(&self, machine: &Machine<D>, cause: CauseRef) -> [u8; 32] {
        let view = match &machine.store.provenance {
            Some(view) => view,
            None => panic!("committed cause must belong to the machine provenance view"),
        };
        let ordinal = view.resolve_ordinal(cause);
        match self.machine_digests.get(ordinal).copied() {
            Some(digest) => digest,
            None => panic!("committed cause ordinal must resolve in its provenance view"),
        }
    }
}

pub(crate) struct CauseDigestIndex {
    records: BTreeMap<[u8; 32], Arc<Vec<u8>>>,
    machine: Vec<[u8; 32]>,
    episodes: Vec<Vec<[u8; 32]>>,
}

impl CauseDigestIndex {
    #[cfg(test)]
    pub(crate) fn from_reference(
        records: BTreeMap<[u8; 32], Vec<u8>>,
        machine: Vec<[u8; 32]>,
        episodes: Vec<Vec<[u8; 32]>>,
    ) -> Self {
        Self {
            records: records
                .into_iter()
                .map(|(digest, payload)| (digest, Arc::new(payload)))
                .collect(),
            machine,
            episodes,
        }
    }
    pub(crate) fn records(&self) -> &BTreeMap<[u8; 32], Arc<Vec<u8>>> {
        &self.records
    }

    pub(crate) fn machine_digest(&self, ordinal: usize) -> [u8; 32] {
        match self.machine.get(ordinal).copied() {
            Some(digest) => digest,
            None => panic!("committed cause ordinal must resolve in its provenance view"),
        }
    }

    pub(crate) fn episode_digest(&self, episode: usize, ordinal: usize) -> [u8; 32] {
        let digests = match self.episodes.get(episode) {
            Some(digests) => digests,
            None => panic!("active episode provenance must be indexed with its episode"),
        };
        match digests.get(ordinal).copied() {
            Some(digest) => digest,
            None => panic!("committed cause ordinal must resolve in its provenance view"),
        }
    }
}

pub(crate) fn cause_digest_index<D>(machine: &Machine<D>) -> CauseDigestIndex {
    let context = ProjectionContext::build(machine);
    let mut reached = context.machine_reachable.clone();
    for episode in &context.episode_reachable {
        reached.extend(episode.iter().copied());
    }
    let mut records = BTreeMap::new();
    for digest in reached {
        match context.table.payloads.get(&digest) {
            Some(payload) => {
                records.insert(digest, payload.clone());
            }
            None => panic!("reachable provenance digest must retain its canonical record"),
        }
    }
    CauseDigestIndex {
        records,
        machine: context.machine_digests,
        episodes: context.episode_digests,
    }
}

fn machine_roots<D>(machine: &Machine<D>) -> Vec<CauseRef> {
    let mut roots = Vec::new();
    roots.extend(machine.store.operation_causes.iter().copied());
    roots.extend(
        machine
            .store
            .standard_causes
            .values()
            .flat_map(|facts| facts.retained_causes()),
    );
    roots.extend(
        machine
            .store
            .active_episodes
            .values()
            .map(|episode| episode.cause()),
    );

    roots.extend(machine.store.input_causes.values().copied());
    roots.extend(machine.store.output_causes.values().copied());
    roots.extend(machine.store.edge_observation_causes.values().copied());
    roots.extend(machine.store.toggle_inversion_causes.values().copied());
    roots.extend(machine.store.establishment_causes.values().copied());
    roots.extend(machine.store.transport_transition_causes.values().copied());
    roots.extend(machine.store.inertial_cancellation_causes.values().copied());
    roots.extend(machine.store.periodic_anchor_causes.values().copied());
    roots.extend(machine.store.periodic_cancellation_causes.values().copied());
    for batch in machine.store.pending_events.values() {
        for event in batch {
            roots.push(event.identity().5);
        }
    }
    roots
}

#[derive(Default)]
struct ContentTable {
    payloads: BTreeMap<[u8; 32], Arc<Vec<u8>>>,
}

impl ContentTable {
    fn insert(&mut self, digest: [u8; 32], payload: Arc<Vec<u8>>) {
        match self.payloads.get(&digest) {
            Some(existing) if existing != &payload => {
                panic!(
                    "digest collision: two different canonical provenance records share one content digest"
                );
            }
            Some(_) => {}
            None => {
                self.payloads.insert(digest, payload);
            }
        }
    }
}

fn index_view<D>(
    compiled: &CompiledNetwork<D>,
    view: &ProvenanceView<D>,
    table: &mut ContentTable,
) -> Vec<[u8; 32]> {
    let mut memo = vec![None; view.records().len()];
    let mut stack = vec![false; view.records().len()];
    for index in 0..view.records().len() {
        digest_record(compiled, view, index, &mut memo, &mut stack, table);
    }
    memo.into_iter()
        .map(|digest| match digest {
            Some(digest) => digest,
            None => panic!("provenance record digest must be computed"),
        })
        .collect()
}

/// Freeze creating-topology bytes before a prepared graph can be published.
pub(crate) fn freeze_provenance<D>(compiled: &CompiledNetwork<D>, view: &ProvenanceView<D>) {
    // SPEC: docs/specs/contracts/machine-state-digests.yaml "content-addressed-provenance"
    // Old nodes keep their creating topology's canonical interpretation.
    let mut table = ContentTable::default();
    #[cfg(test)]
    view.records().freeze_sources(compiled);
    index_view(compiled, view, &mut table);
}

#[cfg(test)]
pub(crate) fn cause_content<D>(view: &ProvenanceView<D>, cause: CauseRef) -> ([u8; 32], Vec<u8>) {
    let Some(canonical) = view.records().canonical(view.resolve_ordinal(cause)).get() else {
        panic!("published causes must retain frozen canonical content");
    };
    (canonical.digest, canonical.payload.as_ref().clone())
}

pub(crate) fn checkpoint_facts<D>(
    compiled: &CompiledNetwork<D>,
    view: &ProvenanceView<D>,
) -> Vec<Vec<u8>> {
    #[cfg(test)]
    crate::state_digest_reference::assert_view(compiled, view);
    let mut table = ContentTable {
        payloads: BTreeMap::new(),
    };
    let digests = index_view(compiled, view, &mut table);
    digests
        .iter()
        .zip(view.records().iter())
        .map(|(digest, record)| {
            let payload = match table.payloads.get(digest) {
                Some(payload) => payload,
                None => panic!("indexed provenance record must have its canonical payload"),
            };
            let mut fact = Record::new();
            fact.field("record", |writer| writer.nested(payload));
            match record {
                ProvenanceRecord::PendingPulseDelay {
                    event,
                    origin,
                    deadline,
                    ..
                }
                | ProvenanceRecord::PendingTransportDelay {
                    event,
                    origin,
                    deadline,
                    ..
                }
                | ProvenanceRecord::PendingInertialDelay {
                    event,
                    origin,
                    deadline,
                    ..
                }
                | ProvenanceRecord::PendingPeriodicBoundary {
                    event,
                    origin,
                    deadline,
                    ..
                } => {
                    fact.field("event", |writer| writer.uint(event.value()));
                    fact.field("origin", |writer| writer.uint(origin.ticks()));
                    fact.field("deadline", |writer| writer.uint(deadline.ticks()));
                }
                _ => {}
            }
            fact.finish()
        })
        .collect()
}

fn digest_record<D>(
    compiled: &CompiledNetwork<D>,
    view: &ProvenanceView<D>,
    index: usize,
    memo: &mut [Option<[u8; 32]>],
    stack: &mut [bool],
    table: &mut ContentTable,
) -> [u8; 32] {
    if let Some(digest) = memo[index] {
        return digest;
    }
    if let Some(canonical) = view.records().canonical(index).get() {
        table.insert(canonical.digest, Arc::clone(&canonical.payload));
        memo[index] = Some(canonical.digest);
        return canonical.digest;
    }
    if stack[index] {
        panic!("provenance records must form an acyclic cause graph");
    }
    stack[index] = true;
    let relations = predecessor_relations(compiled, view, index, memo, stack, table);
    let payload = Arc::new(provenance_payload(
        compiled,
        &view.records()[index],
        &relations,
    ));
    #[cfg(test)]
    crate::causal_work::update(|work| {
        work.canonical_encodes += 1;
        work.canonical_hashes += 1;
    });
    let input = domain_separated(PROVENANCE_RECORD_DOMAIN, 3, &payload);
    let digest = *blake3::hash(&input).as_bytes();
    let canonical = crate::causal_store::CanonicalRecord {
        digest,
        payload: Arc::clone(&payload),
    };
    if let Err(existing) = view.records().canonical(index).set(canonical) {
        let Some(frozen) = view.records().canonical(index).get() else {
            panic!("a concurrently frozen causal node must retain its canonical content");
        };
        if frozen.digest != existing.digest || frozen.payload != existing.payload {
            panic!(
                "an immutable causal node must have one source-qualified canonical interpretation"
            );
        }
    }
    table.insert(digest, payload);
    memo[index] = Some(digest);
    stack[index] = false;
    digest
}

struct Relation {
    role: &'static str,
    digest: [u8; 32],
    payload: Vec<u8>,
    contribution_order: Option<(Vec<u8>, usize)>,
}

fn predecessor_relations<D>(
    compiled: &CompiledNetwork<D>,
    view: &ProvenanceView<D>,
    index: usize,
    memo: &mut [Option<[u8; 32]>],
    stack: &mut [bool],
    table: &mut ContentTable,
) -> Vec<Relation> {
    let record = &view.records()[index];
    let mut relations = Vec::new();
    if let Some(contributions) = contributions(record) {
        for (position, contribution) in contributions.iter().enumerate() {
            let ordinal = view.resolve_ordinal(contribution.cause());
            let digest = digest_record(compiled, view, ordinal, memo, stack, table);
            let mut port = Cbor::default();
            write_pulse_port(&mut port, contribution.port());
            let port_bytes = port.finish();
            let mut payload = Record::new();
            let count = contribution.count().get();
            payload.field("count", |writer| writer.uint(count));
            payload.field("port", |writer| writer.nested(&port_bytes));
            relations.push(Relation {
                role: "contribution",
                digest,
                payload: payload.finish(),
                contribution_order: Some((port_bytes, position)),
            });
        }
    }
    for cause in record.supporters() {
        let ordinal = view.resolve_ordinal(*cause);
        let digest = digest_record(compiled, view, ordinal, memo, stack, table);
        relations.push(Relation {
            role: "supporter",
            digest,
            payload: Vec::new(),
            contribution_order: None,
        });
    }
    let mut contributions = Vec::new();
    let mut supporters = Vec::new();
    for relation in relations {
        if relation.contribution_order.is_some() {
            contributions.push(relation);
        } else {
            supporters.push(relation);
        }
    }
    contributions.sort_by(|left, right| left.contribution_order.cmp(&right.contribution_order));
    supporters.sort_by(|left, right| {
        (left.role, left.digest, &left.payload).cmp(&(right.role, right.digest, &right.payload))
    });
    contributions.append(&mut supporters);
    contributions
}

fn contributions<D>(record: &ProvenanceRecord<D>) -> Option<&[PulseContribution]> {
    match record {
        ProvenanceRecord::PulseDerived { contributions, .. }
        | ProvenanceRecord::PulseControlledLevel { contributions, .. } => Some(contributions),
        _ => None,
    }
}

fn provenance_payload<D>(
    compiled: &CompiledNetwork<D>,
    record: &ProvenanceRecord<D>,
    relations: &[Relation],
) -> Vec<u8> {
    let mut payload = Record::new();
    payload.field("kind", |writer| write_provenance_kind(writer, record));
    if !relations.is_empty() {
        payload.field("predecessors", |writer| {
            writer.array_start(relations.len());
            for relation in relations {
                let mut predecessor = Record::new();
                if relation.payload.is_empty() {
                    predecessor.field("payload", |writer| writer.null());
                } else {
                    let bytes = relation.payload.clone();
                    predecessor.field("payload", |writer| writer.nested(&bytes));
                }
                let digest = relation.digest;
                predecessor.field("predecessor", |writer| writer.bytes(&digest));
                let role = relation.role;
                predecessor.field("role", |writer| writer.text(role));
                writer.nested(&predecessor.finish());
            }
        });
    }
    payload.field("provenance_semantics_version", |writer| writer.uint(3));
    match record {
        ProvenanceRecord::InitializationTransaction { revision, .. }
        | ProvenanceRecord::ReadyTransaction { revision, .. }
        | ProvenanceRecord::PendingPulseDelay { revision, .. }
        | ProvenanceRecord::PendingTransportDelay { revision, .. }
        | ProvenanceRecord::PendingInertialDelay { revision, .. }
        | ProvenanceRecord::PendingPeriodicBoundary { revision, .. } => {
            let value = revision.value();
            payload.field("revision", |writer| writer.uint(value));
        }
        ProvenanceRecord::ExternalObservation { .. }
        | ProvenanceRecord::ExternalPulseObservation { .. }
        | ProvenanceRecord::Derived { .. }
        | ProvenanceRecord::PulseDerived { .. }
        | ProvenanceRecord::PulseControlledLevel { .. } => {}
        ProvenanceRecord::TopologyChange { revision, .. } => {
            payload.field("revision", |writer| writer.uint(revision.value()));
        }
        ProvenanceRecord::Migration { .. } | ProvenanceRecord::Checkpoint { .. } => {}
    }
    if let Some(subject) = provenance_subject_bytes(compiled, record) {
        payload.field("subject", |writer| writer.nested(&subject));
    }
    if let Some(stamp) = provenance_stamp(record) {
        payload.field("reaction_order", |writer| writer.uint(stamp.order()));
        payload.field("stimulus_time", |writer| writer.uint(stamp.time().ticks()));
    }
    if let Some(time) = provenance_time(record) {
        payload.field("time", |writer| writer.uint(time));
    }
    payload.finish()
}

fn write_provenance_kind<D>(writer: &mut Cbor, record: &ProvenanceRecord<D>) {
    match record {
        ProvenanceRecord::TopologyChange { base, target, .. } => {
            writer.variant_start("topology_change");
            let mut body = Record::new();
            body.field("base", |writer| writer.bytes(&base.as_bytes()));
            body.field("target", |writer| writer.bytes(&target.as_bytes()));
            writer.nested(&body.finish());
        }
        ProvenanceRecord::Migration { rule, .. } => {
            writer.variant_start("migration");
            writer.text(rule);
        }
        ProvenanceRecord::Checkpoint { fact, .. } => {
            writer.variant_start("checkpoint");
            writer.bytes(fact);
        }
        ProvenanceRecord::InitializationTransaction { .. } => {
            writer.variant_null("initialization_transaction");
        }
        ProvenanceRecord::ReadyTransaction { .. } => writer.variant_null("ready_transaction"),
        ProvenanceRecord::ExternalObservation { value, .. } => {
            writer.variant_start("external_observation");
            logic_level(writer, *value);
        }
        ProvenanceRecord::ExternalPulseObservation { count, .. } => {
            writer.variant_start("external_pulse_observation");
            writer.uint(count.get());
        }
        ProvenanceRecord::PendingPulseDelay { count, .. } => {
            writer.variant_start("pending_pulse_delay");
            let mut body = Record::new();
            let count = count.get();
            body.field("count", |writer| writer.uint(count));
            writer.nested(&body.finish());
        }
        ProvenanceRecord::PendingTransportDelay { target, .. }
        | ProvenanceRecord::PendingInertialDelay { target, .. } => {
            let name = match record {
                ProvenanceRecord::PendingTransportDelay { .. } => "pending_transport_delay",
                _ => "pending_inertial_delay",
            };
            writer.variant_start(name);
            let mut body = Record::new();
            let target = *target;
            body.field("target", |writer| logic_level(writer, target));
            writer.nested(&body.finish());
        }
        ProvenanceRecord::PendingPeriodicBoundary {
            anchor,
            ordinal,
            first_emission,
            reenable_phase,
            ..
        } => {
            writer.variant_start("pending_periodic_boundary");
            let mut body = Record::new();
            let anchor = anchor.ticks();
            let ordinal = *ordinal;
            let first_emission = *first_emission;
            let reenable_phase = *reenable_phase;
            body.field("anchor", |writer| writer.uint(anchor));
            body.field("first_emission", |writer| {
                write_first_emission(writer, first_emission)
            });
            body.field("ordinal", |writer| writer.uint(ordinal));
            body.field("reenable_phase", |writer| {
                write_reenable_phase(writer, reenable_phase)
            });
            writer.nested(&body.finish());
        }
        ProvenanceRecord::Derived { .. } => writer.variant_null("derived"),
        ProvenanceRecord::PulseDerived { result, .. } => {
            writer.variant_start("pulse_derived");
            writer.uint(result.get());
        }
        ProvenanceRecord::PulseControlledLevel { result, .. } => {
            writer.variant_start("pulse_controlled_level");
            logic_level(writer, *result);
        }
    }
}

fn provenance_subject_bytes<D>(
    compiled: &CompiledNetwork<D>,
    record: &ProvenanceRecord<D>,
) -> Option<Vec<u8>> {
    let mut writer = Cbor::default();
    match record {
        ProvenanceRecord::ExternalObservation { input, .. } => {
            writer.variant_start("external_input");
            writer.key(input.as_u128());
        }
        ProvenanceRecord::ExternalPulseObservation { input, .. } => {
            writer.variant_start("external_pulse_input");
            writer.key(input.as_u128());
        }
        ProvenanceRecord::PendingPulseDelay { owner, .. }
        | ProvenanceRecord::PendingTransportDelay { owner, .. }
        | ProvenanceRecord::PendingInertialDelay { owner, .. }
        | ProvenanceRecord::PendingPeriodicBoundary { owner, .. } => {
            write_node_subject(&mut writer, owner);
        }
        ProvenanceRecord::Derived { subject, .. }
        | ProvenanceRecord::PulseDerived { subject, .. }
        | ProvenanceRecord::PulseControlledLevel { subject, .. } => {
            write_provenance_subject(&mut writer, compiled, subject);
        }
        ProvenanceRecord::InitializationTransaction { .. }
        | ProvenanceRecord::ReadyTransaction { .. } => return None,
        ProvenanceRecord::Migration { subject, .. } => {
            write_provenance_subject(&mut writer, compiled, subject)
        }
        ProvenanceRecord::TopologyChange { .. } | ProvenanceRecord::Checkpoint { .. } => {
            return None;
        }
    }
    Some(writer.finish())
}

fn provenance_stamp<D>(record: &ProvenanceRecord<D>) -> Option<crate::ReactionStamp<D>> {
    match record {
        ProvenanceRecord::InitializationTransaction { at, .. }
        | ProvenanceRecord::ReadyTransaction { at, .. }
        | ProvenanceRecord::TopologyChange { at, .. } => Some(*at),
        ProvenanceRecord::ExternalObservation { stamp, .. }
        | ProvenanceRecord::ExternalPulseObservation { stamp, .. } => Some(*stamp),
        ProvenanceRecord::PendingPulseDelay { stimulus, .. }
        | ProvenanceRecord::PendingTransportDelay { stimulus, .. }
        | ProvenanceRecord::PendingInertialDelay { stimulus, .. }
        | ProvenanceRecord::PendingPeriodicBoundary { stimulus, .. } => Some(*stimulus),
        _ => None,
    }
}
fn provenance_time<D>(record: &ProvenanceRecord<D>) -> Option<u64> {
    match record {
        ProvenanceRecord::PendingPulseDelay { origin, .. }
        | ProvenanceRecord::PendingTransportDelay { origin, .. }
        | ProvenanceRecord::PendingInertialDelay { origin, .. }
        | ProvenanceRecord::PendingPeriodicBoundary { origin, .. } => Some(origin.ticks()),
        _ => provenance_stamp(record).map(|stamp| stamp.time().ticks()),
    }
}

fn reachable_digests<D>(
    view: &ProvenanceView<D>,
    digests: &[[u8; 32]],
    roots: &[CauseRef],
) -> BTreeSet<[u8; 32]> {
    let mut seen = vec![false; view.records().len()];
    let mut pending = Vec::new();
    for cause in roots {
        pending.push(view.resolve_ordinal(*cause));
    }
    let mut reached = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if seen[index] {
            continue;
        }
        seen[index] = true;
        #[cfg(test)]
        crate::causal_work::update(|work| work.closure_records_visited += 1);
        reached.insert(digests[index]);
        for cause in view.records()[index].predecessor_causes() {
            pending.push(view.resolve_ordinal(cause));
        }
    }
    reached
}

fn write_provenance(writer: &mut Cbor, context: &ProjectionContext) {
    let mut digests = context.machine_reachable.clone();
    for reached in &context.episode_reachable {
        digests.extend(reached.iter().copied());
    }
    writer.array_start(digests.len());
    for digest in digests {
        match context.table.payloads.get(&digest) {
            Some(payload) => {
                #[cfg(test)]
                crate::causal_work::update(|work| {
                    work.closure_records_emitted += 1;
                    work.closure_bytes_emitted += payload.len();
                });
                writer.nested(payload);
            }
            None => panic!("reachable provenance digest must retain its canonical record"),
        }
    }
}

fn write_lifecycle<D>(writer: &mut Cbor, machine: &Machine<D>) {
    match machine.store.status {
        crate::machine::MachineStatus::AwaitingInitialization => {
            writer.variant_null("awaiting_initialization");
        }
        crate::machine::MachineStatus::Ready { now } => {
            writer.variant_start("ready");
            let mut body = Record::new();
            let mut levels: Vec<_> = machine
                .store
                .external_levels
                .iter()
                .map(|(key, level)| (key.as_u128(), *level))
                .collect();
            levels.sort_by_key(|(key, _)| *key);
            body.field("external_levels", |writer| {
                writer.array_start(levels.len());
                for (key, level) in levels {
                    writer.array_start(2);
                    writer.key(key);
                    logic_level(writer, level);
                }
            });
            let ticks = now.ticks();
            body.field("time", |writer| writer.uint(ticks));
            let order = match machine.last_reaction() {
                Some(stamp) if stamp.time() == now => stamp.order(),
                _ => {
                    panic!("ready machine must retain its committed reaction stamp at current time")
                }
            };
            body.field("reaction_order", |writer| writer.uint(order));
            writer.nested(&body.finish());
        }
    }
}

fn write_state<D>(writer: &mut Cbor, machine: &Machine<D>) {
    let mut entries = Vec::new();
    for slot in machine.compiled.state_slots() {
        let mut record = Record::new();
        let owner = owner_bytes(&slot.owner);
        record.field("owner", |writer| writer.nested(&owner));
        let schema = match slot.family {
            DigestStateFamily::Edge { .. } => "edge_observation",
            DigestStateFamily::StoredLevel { .. } => "stored_level",
            DigestStateFamily::Transport { .. } => "transport_level",
            DigestStateFamily::Inertial { .. } => "inertial_level",
            DigestStateFamily::Periodic { .. } => "periodic_enable",
        };
        record.field("schema", |writer| writer.variant_null(schema));
        let value = state_value(machine, &slot);
        record.field("value", |writer| writer.nested(&value));
        entries.push((owner, record.finish()));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    writer.array_start(entries.len());
    for (_, entry) in entries {
        writer.nested(&entry);
    }
}

fn state_value<D>(machine: &Machine<D>, slot: &crate::compile::DigestStateSlot) -> Vec<u8> {
    let mut writer = Cbor::default();
    match slot.family {
        DigestStateFamily::Edge { index } => {
            let observation = stored_edge(machine, index);
            match observation {
                EdgeObservation::Unestablished => writer.variant_null("unestablished"),
                EdgeObservation::Established(level) => {
                    writer.variant_start("established");
                    logic_level(&mut writer, level);
                }
            }
        }
        DigestStateFamily::StoredLevel { index } => {
            logic_level(&mut writer, stored_level(machine, index));
        }
        DigestStateFamily::Transport { remembered, output } => {
            let mut body = Record::new();
            let output = stored_level(machine, output);
            let remembered = stored_level(machine, remembered);
            body.field("output", |writer| logic_level(writer, output));
            body.field("remembered_input", |writer| logic_level(writer, remembered));
            return body.finish();
        }
        DigestStateFamily::Inertial { remembered, output } => {
            let mut body = Record::new();
            if let Some(candidate) = singular_event(machine, slot.flat, "inertial_delay") {
                body.field("candidate", |writer| writer.uint(candidate));
            }
            let output = stored_level(machine, output);
            let remembered = stored_level(machine, remembered);
            body.field("output", |writer| logic_level(writer, output));
            body.field("remembered_input", |writer| logic_level(writer, remembered));
            return body.finish();
        }
        DigestStateFamily::Periodic { previous_enable } => {
            let mut body = Record::new();
            if let Some(anchor) = machine.store.periodic_anchors.get(&slot.flat).copied() {
                let ticks = anchor.anchor.ticks();
                body.field("anchor", |writer| writer.uint(ticks));
                body.field("phase_time", |writer| {
                    writer.uint(anchor.origin.time().ticks())
                });
                body.field("phase_order", |writer| writer.uint(anchor.origin.order()));
                if let Some(settled) = anchor.settled {
                    body.field("settled_boundary", |writer| writer.uint(settled.ticks()));
                }
            }
            if let Some(boundary) = singular_event(machine, slot.flat, "periodic") {
                body.field("next_boundary", |writer| writer.uint(boundary));
            }
            let previous = stored_level(machine, previous_enable);
            body.field("previous_enable", |writer| logic_level(writer, previous));
            return body.finish();
        }
    }
    writer.finish()
}

fn singular_event<D>(machine: &Machine<D>, node: crate::key::NodeKey, kind: &str) -> Option<u64> {
    let mut keys = Vec::new();
    for event in machine.store.pending_events.values().flatten() {
        if event.identity().1 == node && event.kind_name() == kind {
            keys.push(event.identity().0.value());
        }
    }
    if keys.len() == 1 { keys.pop() } else { None }
}

fn write_pending_events<D>(writer: &mut Cbor, machine: &Machine<D>, context: &ProjectionContext) {
    let mut events = Vec::new();
    for event in machine.store.pending_events.values().flatten().copied() {
        let (key, node, origin, deadline, revision, cause) = event.identity();
        let owner = owner_bytes(&machine.compiled.stable_owner(node));
        let mut record = Record::new();
        let cause = context.machine_cause(machine, cause);
        record.field("cause", |writer| writer.bytes(&cause));
        let deadline = deadline.ticks();
        record.field("deadline", |writer| writer.uint(deadline));
        let serial = key.value();
        record.field("key", |writer| writer.uint(serial));
        let kind = pending_kind_bytes(event);
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
        events.push((
            deadline,
            owner,
            event.kind_name().to_string(),
            serial,
            record.finish(),
        ));
    }
    events.sort_by(|left, right| {
        (left.0, &left.1, &left.2, left.3).cmp(&(right.0, &right.1, &right.2, right.3))
    });
    writer.array_start(events.len());
    for (_, _, _, _, event) in events {
        writer.nested(&event);
    }
}

fn pending_kind_bytes<D>(event: PendingEvent<D>) -> Vec<u8> {
    let mut writer = Cbor::default();
    match event {
        PendingEvent::PulseDelay(event) => {
            writer.variant_start("pulse_delay");
            let mut body = Record::new();
            let count = event.count.get();
            body.field("count", |writer| writer.uint(count));
            writer.nested(&body.finish());
        }
        PendingEvent::TransportDelay(event) => {
            writer.variant_start("transport_delay");
            let mut body = Record::new();
            body.field("target", |writer| logic_level(writer, event.target));
            writer.nested(&body.finish());
        }
        PendingEvent::Inertial(event) => {
            writer.variant_start("inertial_delay");
            let mut body = Record::new();
            body.field("target", |writer| logic_level(writer, event.target));
            writer.nested(&body.finish());
        }
        PendingEvent::Periodic(event) => {
            writer.variant_start("periodic");
            let mut body = Record::new();
            let anchor = event.anchor.ticks();
            let ordinal = event.ordinal;
            body.field("anchor", |writer| writer.uint(anchor));
            body.field("first_emission", |writer| {
                write_first_emission(writer, event.first_emission)
            });
            body.field("ordinal", |writer| writer.uint(ordinal));
            body.field("reenable_phase", |writer| {
                write_reenable_phase(writer, event.reenable_phase)
            });
            writer.nested(&body.finish());
        }
    }
    writer.finish()
}

fn write_episodes<D>(writer: &mut Cbor, machine: &Machine<D>, context: &ProjectionContext) {
    let mut episodes = Vec::new();
    for (index, episode) in machine.store.active_episodes.values().enumerate() {
        let identity = episode.identity().as_bytes();
        let digests = &context.episode_digests[index];
        let view = episode.provenance();
        let ordinal = view.resolve_ordinal(episode.cause());
        let cause = digests[ordinal];
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
        let evidence = evidence_bytes(episode.current());
        record.field("evidence", |writer| writer.nested(&evidence));
        record.field("identity", |writer| writer.bytes(&identity));
        let changed = episode.last_material_change().ticks();
        record.field("last_material_change", |writer| writer.uint(changed));
        record.field("last_material_order", |writer| {
            writer.uint(episode.last_material_stamp().order())
        });
        let owner = node_subject_bytes(episode.condition().owner());
        record.field("owner", |writer| writer.nested(&owner));
        episodes.push((identity, record.finish()));
    }
    episodes.sort_by_key(|(identity, _)| *identity);
    writer.array_start(episodes.len());
    for (_, episode) in episodes {
        writer.nested(&episode);
    }
}

fn evidence_bytes<D>(problem: &Problem<D>) -> Vec<u8> {
    let evidence = match problem.evidence() {
        ProblemEvidence::RuntimeLevelLatchConflictRetained { evidence, .. } => evidence,
        _ => panic!("active episode evidence must be a retained level conflict"),
    };
    let mut record = Record::new();
    let controls = evidence.controls;
    record.field("controls", |writer| write_controls(writer, controls));
    let policy = evidence.policy;
    record.field("policy", |writer| write_conflict_policy(writer, policy));
    let previous = evidence.previous;
    record.field("previous", |writer| logic_level(writer, previous));
    let revision = evidence.revision.value();
    record.field("revision", |writer| writer.uint(revision));
    record.finish()
}

fn write_baselines<D>(writer: &mut Cbor, machine: &Machine<D>, context: &ProjectionContext) {
    let mut baselines = Vec::new();
    for key in machine.compiled.external_level_outputs() {
        let level = match machine.store.output_baselines.get(&key).copied() {
            Some(level) => level,
            None => panic!("ready external level output must have an established baseline"),
        };
        let cause = match machine.store.output_causes.get(&key).copied() {
            Some(cause) => context.machine_cause(machine, cause),
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
        let subject = endpoint_bytes(&slot.endpoint);
        let mut record = Record::new();
        record.field("level", |writer| logic_level(writer, level));
        record.field("subject", |writer| writer.nested(&subject));
        facts.push((subject, record.finish()));
    }
    facts.sort_by(|left, right| left.0.cmp(&right.0));
    writer.array_start(facts.len());
    for (_, fact) in facts {
        writer.nested(&fact);
    }
}

fn endpoint_bytes(endpoint: &SettledEndpoint) -> Vec<u8> {
    let mut writer = Cbor::default();
    match endpoint {
        SettledEndpoint::NodeInput { owner, port } => {
            write_port_subject(&mut writer, "node_input", "module_node_input", owner, *port);
        }
        SettledEndpoint::NodeOutput { owner, port } => {
            write_port_subject(
                &mut writer,
                "node_output",
                "module_node_output",
                owner,
                *port,
            );
        }
        SettledEndpoint::ExternalOutput(key) => {
            writer.variant_start("external_output");
            writer.key(*key);
        }
    }
    writer.finish()
}

fn write_port_subject(
    writer: &mut Cbor,
    top_level: &str,
    module_level: &str,
    owner: &StableOwner,
    port: u128,
) {
    if owner.instances.is_empty() {
        writer.variant_start(top_level);
        let mut body = Record::new();
        let node = owner.node.as_u128();
        body.field("node", |writer| writer.key(node));
        body.field("port", |writer| writer.key(port));
        writer.nested(&body.finish());
    } else {
        writer.variant_start(module_level);
        let mut body = Record::new();
        let node = owner.node.as_u128();
        body.field("instances", |writer| {
            write_instance_path(writer, &owner.instances);
        });
        body.field("node", |writer| writer.key(node));
        body.field("port", |writer| writer.key(port));
        writer.nested(&body.finish());
    }
}

fn write_provenance_subject<D>(
    writer: &mut Cbor,
    compiled: &CompiledNetwork<D>,
    subject: &crate::transaction::ProvenanceSubject,
) {
    match subject.kind() {
        ProvenanceSubjectKind::Node(node) => {
            write_stable_owner(writer, &compiled.stable_owner(node))
        }
        ProvenanceSubjectKind::Qualified(node) => write_qualified_node(writer, node),
        ProvenanceSubjectKind::ExternalOutput(output) => {
            writer.variant_start("external_output");
            writer.key(output.as_u128());
        }
        ProvenanceSubjectKind::PulseExternalOutput(output) => {
            writer.variant_start("external_pulse_output");
            writer.key(output.as_u128());
        }
    }
}

fn write_node_subject(writer: &mut Cbor, subject: &NodeSubject) {
    match subject {
        NodeSubject::Node(node) => {
            writer.variant_start("node");
            writer.key(node.as_u128());
        }
        NodeSubject::Qualified(node) => write_qualified_node(writer, node),
    }
}

fn write_qualified_node(writer: &mut Cbor, node: &QualifiedNodeRef) {
    writer.variant_start("module_node");
    let mut body = Record::new();
    let local = node.node().as_u128();
    body.field("instances", |writer| {
        write_instance_path(writer, node.instances());
    });
    body.field("node", |writer| writer.key(local));
    writer.nested(&body.finish());
}

fn write_stable_owner(writer: &mut Cbor, owner: &StableOwner) {
    if owner.instances.is_empty() {
        writer.variant_start("node");
        writer.key(owner.node.as_u128());
    } else {
        writer.variant_start("module_node");
        let mut body = Record::new();
        let node = owner.node.as_u128();
        body.field("instances", |writer| {
            write_instance_path(writer, &owner.instances);
        });
        body.field("node", |writer| writer.key(node));
        writer.nested(&body.finish());
    }
}

fn write_pulse_port(writer: &mut Cbor, port: &PulsePortSubject) {
    match port {
        PulsePortSubject::Port(port) => {
            writer.variant_start("in_port");
            writer.key(port.as_u128());
        }
        PulsePortSubject::Qualified(port) => {
            let crate::key::AnyInPortKey::Pulse(local) = port.port() else {
                panic!("pulse contribution port must be a pulse input");
            };
            writer.variant_start("module_in_port");
            let mut body = Record::new();
            let local = local.as_u128();
            body.field("instances", |writer| {
                write_instance_path(writer, port.instances());
            });
            body.field("port", |writer| writer.key(local));
            writer.nested(&body.finish());
        }
    }
}

pub(crate) fn encode_stable_owner(owner: &StableOwner) -> Vec<u8> {
    owner_bytes(owner)
}

pub(crate) fn encode_settled_endpoint(endpoint: &SettledEndpoint) -> Vec<u8> {
    endpoint_bytes(endpoint)
}

pub(crate) fn encode_node_subject(subject: &NodeSubject) -> Vec<u8> {
    node_subject_bytes(subject)
}

pub(crate) fn encode_episode_evidence<D>(problem: &crate::diagnostics::Problem<D>) -> Vec<u8> {
    evidence_bytes(problem)
}

pub(crate) fn episode_evidence_revision<D>(problem: &crate::diagnostics::Problem<D>) -> u64 {
    match problem.evidence() {
        crate::diagnostics::ProblemEvidence::RuntimeLevelLatchConflictRetained {
            evidence, ..
        } => evidence.revision.value(),
        _ => panic!("active episode evidence must be a retained level conflict"),
    }
}

fn owner_bytes(owner: &StableOwner) -> Vec<u8> {
    let mut writer = Cbor::default();
    write_stable_owner(&mut writer, owner);
    writer.finish()
}

// SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "61. Nested modules"
// Nested correspondence keeps the outer instance, nested instance, and internal key.
fn write_instance_path(writer: &mut Cbor, instances: &[ModuleInstanceKey]) {
    if instances.is_empty() {
        panic!("qualified module identity must retain a non-empty instance path");
    }
    writer.array_start(instances.len());
    for instance in instances {
        writer.key(instance.as_u128());
    }
}

fn node_subject_bytes(subject: &NodeSubject) -> Vec<u8> {
    let mut writer = Cbor::default();
    write_node_subject(&mut writer, subject);
    writer.finish()
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

fn write_controls(writer: &mut Cbor, controls: ConflictControls) {
    match controls {
        ConflictControls::Level { set, reset } => {
            writer.variant_start("level");
            let mut body = Record::new();
            body.field("reset", |writer| logic_level(writer, reset));
            body.field("set", |writer| logic_level(writer, set));
            writer.nested(&body.finish());
        }
        ConflictControls::Pulse { set, reset } => {
            writer.variant_start("pulse");
            let mut body = Record::new();
            let reset = reset.get();
            let set = set.get();
            body.field("reset", |writer| writer.uint(reset));
            body.field("set", |writer| writer.uint(set));
            writer.nested(&body.finish());
        }
    }
}

fn write_conflict_policy(writer: &mut Cbor, policy: ConflictPolicy) {
    writer.variant_null(match policy {
        ConflictPolicy::SetDominant => "set_dominant",
        ConflictPolicy::ResetDominant => "reset_dominant",
        ConflictPolicy::RetainAndDiagnose => "retain_and_diagnose",
        ConflictPolicy::RejectTransaction => "reject_transaction",
    });
}

fn write_first_emission(writer: &mut Cbor, policy: FirstEmissionPolicy) {
    writer.variant_null(match policy {
        FirstEmissionPolicy::Immediate => "immediate",
        FirstEmissionPolicy::AfterFirstPeriod => "after_first_period",
    });
}

fn write_reenable_phase(writer: &mut Cbor, policy: ReenablePhasePolicy) {
    writer.variant_null(match policy {
        ReenablePhasePolicy::RestartPhase => "restart_phase",
        ReenablePhasePolicy::PreservePhase => "preserve_phase",
    });
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
    use super::*;
    use crate::authored::{
        ConflictPolicy, ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef,
        InputPortRole, LevelSetResetConfig, ModuleBinding, ModuleBindingSet, ModuleInputDef,
        ModuleInstanceDef, ModuleInterfaceMapping, ModuleOutputDef, NodeDef, NodeKind, NodePorts,
        UncheckedModule, UncheckedNetwork,
    };
    use crate::key::{
        ConnectionKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
        ModuleInstanceKey, ModuleOutputKey, NetworkKey, NodeKey, OutPortKey, SignalSourceKey,
    };
    use crate::machine::NetworkRevision;
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
    use crate::time::{NonZeroSpan, Time};
    use crate::{
        ModuleBuilder, NetworkBuilder, RuntimePolicy, TimeDomainId, ToggleConfig, Transaction,
    };
    use std::fs;
    use std::path::PathBuf;

    fn policy(values: [u64; 5]) -> RuntimePolicy {
        match RuntimePolicy::builder()
            .max_internal_reactions(values[0])
            .max_evaluated_operations(values[1])
            .max_pending_events(values[2])
            .max_events_created_per_transaction(values[3])
            .max_required_provenance_growth(values[4])
            .build()
        {
            Ok(policy) => policy,
            Err(failure) => panic!("complete policy must build: {failure}"),
        }
    }

    fn generous_policy() -> RuntimePolicy {
        policy([100, 10_000, 100, 100, 10_000])
    }

    fn span(ticks: u64) -> NonZeroSpan<()> {
        match NonZeroSpan::from_ticks(ticks) {
            Ok(span) => span,
            Err(failure) => panic!("fixture span must be positive: {failure}"),
        }
    }

    fn compile(network: UncheckedNetwork<()>) -> crate::CompiledNetwork<()> {
        let validated = match network.validate().require_artifact() {
            Ok(network) => network,
            Err(_) => panic!("fixture must validate"),
        };
        match validated.compile().require_artifact() {
            Ok(compiled) => compiled,
            Err(_) => panic!("fixture must compile"),
        }
    }

    fn golden_toggle(meta: DiagnosticMeta) -> UncheckedNetwork<()> {
        let input = InPortKey::<Pulse>::from_u128(4);
        let output = OutPortKey::<Level>::from_u128(5);
        let external = ExternalInputKey::<Pulse>::from_u128(6);
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::toggle(LogicLevel::Low),
                NodePorts::with_input_roles(
                    vec![input.into()],
                    vec![InputPortRole::Toggle],
                    vec![output.into()],
                ),
                meta,
            )],
            vec![ExternalInputDef::new(
                external.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(8).into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                external.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        )
    }

    fn spawn(network: UncheckedNetwork<()>, limits: [u64; 5]) -> Machine<()> {
        compile(network).spawn(policy(limits))
    }

    fn initialize(machine: &mut Machine<()>, at: u64) -> crate::TransactionResult<()> {
        let snapshot = match machine.compiled().input_snapshot().finish() {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("fixture snapshot must bind: {failure}"),
        };
        match machine.apply(Transaction::initialize(
            Time::from_ticks(at),
            machine.revision(),
            snapshot,
        )) {
            Ok(result) => result,
            Err(failure) => panic!("fixture initialization must commit: {failure}"),
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

    fn golden_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name)
    }

    fn assert_golden(name: &str, bytes: &[u8]) {
        let path = golden_path(name);
        let actual = hex(bytes);
        let expected = fs::read_to_string(&path)
            .unwrap_or_else(|failure| panic!("missing golden {}: {failure}", path.display()));
        assert_eq!(actual, expected.trim(), "golden {}", path.display());
    }

    fn key_image(value: u128) -> Vec<u8> {
        let mut bytes = vec![0x50];
        bytes.extend(value.to_be_bytes());
        bytes
    }

    #[test]
    fn uninitialized_and_ready_digests_match_versioned_goldens() {
        let mut machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        assert_eq!(machine.revision().value(), 0);
        assert_golden(
            "execution_state_uninitialized.hex",
            &execution_digest_input(&machine, 3, 3),
        );
        assert_golden(
            "observable_state_uninitialized.hex",
            &observable_digest_input(&machine, 3, 3),
        );
        let uninitialized_execution = machine.execution_state_digest();
        let result = initialize(&mut machine, 1);
        assert_eq!(result.before_execution_digest(), uninitialized_execution);
        assert_eq!(
            result.after_execution_digest(),
            machine.execution_state_digest()
        );
        assert_eq!(
            result.after_observable_digest(),
            machine.observable_state_digest()
        );
        assert_ne!(
            result.before_execution_digest().as_bytes(),
            result.after_execution_digest().as_bytes()
        );
        assert_golden(
            "execution_state_ready.hex",
            &execution_digest_input(&machine, 3, 3),
        );
        assert_golden(
            "observable_state_ready.hex",
            &observable_digest_input(&machine, 3, 3),
        );
        assert_golden(
            "execution_state_digest_ready.hex",
            &machine_pair_bytes(&machine),
        );
        assert_golden(
            "execution_state_digest_uninitialized.hex",
            &machine_pair_bytes(&spawn(
                golden_toggle(DiagnosticMeta::default()),
                [8, 100, 4, 8, 100],
            )),
        );
    }

    fn machine_pair_bytes(machine: &Machine<()>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(machine.execution_state_digest().as_bytes());
        bytes.extend(machine.observable_state_digest().as_bytes());
        bytes
    }

    #[test]
    fn uninitialized_projection_has_no_fabricated_ready_facts() {
        let machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        let execution = execution_digest_input(&machine, 3, 3);
        let observable = observable_digest_input(&machine, 3, 3);
        assert!(
            execution
                .windows(b"awaiting_initialization".len())
                .any(|window| window == b"awaiting_initialization")
        );
        for absent in [
            "external_levels",
            "settled_levels",
            "output_baselines",
            "provenance",
            "explanation_boundary",
        ] {
            assert!(
                !execution
                    .windows(absent.len())
                    .any(|window| window == absent.as_bytes()),
                "{absent} is not an uninitialized execution fact"
            );
            assert!(
                !observable
                    .windows(absent.len())
                    .any(|window| window == absent.as_bytes()),
                "{absent} is not an uninitialized observable fact"
            );
        }
        assert_ne!(
            machine.execution_state_digest().as_bytes(),
            machine.observable_state_digest().as_bytes()
        );
        let execution_payload = projection_payload(&machine, 1, false);
        let observable_payload = projection_payload(&machine, 1, true);
        assert_eq!(execution_payload, observable_payload);
    }

    #[test]
    fn identical_payloads_under_distinct_domains_differ() {
        let machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        let payload = projection_payload(&machine, 1, false);
        let execution = domain_separated(EXECUTION_STATE_DOMAIN, 2, &payload);
        let observable = domain_separated(OBSERVABLE_STATE_DOMAIN, 2, &payload);
        assert_ne!(execution, observable);
        assert_ne!(blake3::hash(&execution), blake3::hash(&observable));
    }

    #[test]
    fn version_components_change_the_digest() {
        let machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        let current = execution_digest_input(&machine, 3, 3);
        assert_ne!(current, execution_digest_input(&machine, 2, 1));
        assert_ne!(current, execution_digest_input(&machine, 1, 2));
        assert_ne!(
            machine.execution_state_digest().as_bytes(),
            *blake3::hash(&execution_digest_input(&machine, 1, 2)).as_bytes()
        );
    }

    #[test]
    fn policy_metadata_history_and_inspection_are_excluded() {
        let limits = [8, 100, 4, 8, 100];
        let plain = spawn(golden_toggle(DiagnosticMeta::default()), limits);
        let annotated = spawn(
            golden_toggle(DiagnosticMeta {
                name: Some("toggle".into()),
                description: Some("presentation".into()),
                ..DiagnosticMeta::default()
            }),
            limits,
        );
        let other_policy = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [9, 200, 5, 10, 300],
        );
        assert_eq!(
            plain.execution_state_digest(),
            annotated.execution_state_digest()
        );
        assert_eq!(
            plain.observable_state_digest(),
            annotated.observable_state_digest()
        );
        assert_eq!(
            plain.execution_state_digest(),
            other_policy.execution_state_digest()
        );
        assert_eq!(
            plain.observable_state_digest(),
            other_policy.observable_state_digest()
        );
        assert_ne!(plain.runtime_policy_id(), other_policy.runtime_policy_id());

        let mut ready = spawn(golden_toggle(DiagnosticMeta::default()), limits);
        initialize(&mut ready, 1);
        let execution = ready.execution_state_digest();
        let observable = ready.observable_state_digest();
        let _ = ready.schedule();
        let _ = ready.inspect_toggle(NodeKey::from_u128(3));
        assert_eq!(ready.execution_state_digest(), execution);
        assert_eq!(ready.observable_state_digest(), observable);

        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let (_toggle_key, toggle) = builder.pulse_input("toggle");
        let (reset_key, reset) = builder.level_input("reset");
        builder
            .level_resettable_toggle(toggle, reset, LogicLevel::Low)
            .unwrap_or_else(|failure| panic!("fixture module must author: {failure:?}"));
        let compiled = compile_builder(builder);
        let mut machine = compiled.spawn(generous_policy());
        let snapshot = match compiled
            .input_snapshot()
            .set(reset_key, LogicLevel::Low)
            .and_then(crate::InputSnapshotBuilder::finish)
        {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("fixture snapshot must bind: {failure}"),
        };
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(1),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("fixture initialization must commit: {failure}"));
        assert!(machine.standard_history_len_for_test() > 0);
        let execution = machine.execution_state_digest();
        let observable = machine.observable_state_digest();
        machine.clear_standard_history_for_test();
        assert_eq!(machine.standard_history_len_for_test(), 0);
        assert_eq!(machine.execution_state_digest(), execution);
        assert_eq!(machine.observable_state_digest(), observable);
    }

    fn compile_builder(builder: NetworkBuilder<()>) -> crate::CompiledNetwork<()> {
        match builder.finish().require_artifact() {
            Ok(network) => match network.compile().require_artifact() {
                Ok(compiled) => compiled,
                Err(_) => panic!("fixture must compile"),
            },
            Err(_) => panic!("fixture must validate"),
        }
    }

    #[test]
    fn inconsistent_baseline_tampering_changes_observation_and_fails_restoration() {
        let mut machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        initialize(&mut machine, 1);
        let observable = machine.observable_state_digest();
        machine.set_output_baseline_for_test(
            ExternalOutputKey::<Level>::from_u128(8),
            LogicLevel::High,
        );
        assert_ne!(machine.observable_state_digest(), observable);
        // A mutated baseline is invalid state, not a supported O-only witness.
        assert!(
            machine
                .compiled()
                .restore(machine.snapshot(), machine.policy.clone())
                .is_err()
        );
    }

    #[test]
    fn included_state_and_time_change_the_execution_digest() {
        let low = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        let high = spawn(toggle_with_initial(LogicLevel::High), [8, 100, 4, 8, 100]);
        assert_ne!(low.execution_state_digest(), high.execution_state_digest());
        let mut early = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        let mut later = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        initialize(&mut early, 1);
        initialize(&mut later, 2);
        assert_ne!(
            early.execution_state_digest(),
            later.execution_state_digest()
        );
    }

    fn toggle_with_initial(initial: LogicLevel) -> UncheckedNetwork<()> {
        let input = InPortKey::<Pulse>::from_u128(4);
        let output = OutPortKey::<Level>::from_u128(5);
        let external = ExternalInputKey::<Pulse>::from_u128(6);
        UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::toggle(initial),
                NodePorts::with_input_roles(
                    vec![input.into()],
                    vec![InputPortRole::Toggle],
                    vec![output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            vec![ExternalInputDef::new(
                external.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(8).into(),
                SignalSourceKey::NodeOutput(output).into(),
                DiagnosticMeta::default(),
            )],
            vec![ConnectionDef::new(
                ConnectionKey::from_u128(7),
                external.into(),
                input.into(),
                DiagnosticMeta::default(),
            )],
        )
    }

    #[test]
    fn rejected_transaction_leaves_both_digests_unchanged() {
        let mut machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [8, 100, 4, 8, 100],
        );
        let execution = machine.execution_state_digest();
        let observable = machine.observable_state_digest();
        let snapshot = match machine.compiled().input_snapshot().finish() {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("fixture snapshot must bind: {failure}"),
        };
        let rejected = machine.apply(Transaction::initialize(
            Time::from_ticks(1),
            NetworkRevision::from_value(9),
            snapshot,
        ));
        assert!(rejected.is_err());
        assert_eq!(machine.execution_state_digest(), execution);
        assert_eq!(machine.observable_state_digest(), observable);
        initialize(&mut machine, 1);
        let execution = machine.execution_state_digest();
        let observable = machine.observable_state_digest();
        let snapshot = match machine.compiled().input_snapshot().finish() {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("fixture snapshot must bind: {failure}"),
        };
        assert!(
            machine
                .apply(Transaction::initialize(
                    Time::from_ticks(2),
                    machine.revision(),
                    snapshot,
                ))
                .is_err()
        );
        assert_eq!(machine.execution_state_digest(), execution);
        assert_eq!(machine.observable_state_digest(), observable);
    }

    #[test]
    fn equal_deadline_storage_order_is_not_semantic() {
        let first_input = InPortKey::<Pulse>::from_u128(11);
        let second_input = InPortKey::<Pulse>::from_u128(21);
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![
                pulse_delay_node(10, first_input, 12),
                pulse_delay_node(20, second_input, 22),
            ],
            vec![
                ExternalInputDef::new(
                    ExternalInputKey::<Pulse>::from_u128(13).into(),
                    DiagnosticMeta::default(),
                ),
                ExternalInputDef::new(
                    ExternalInputKey::<Pulse>::from_u128(23).into(),
                    DiagnosticMeta::default(),
                ),
            ],
            Vec::new(),
            vec![
                ConnectionDef::new(
                    ConnectionKey::from_u128(14),
                    ExternalInputKey::<Pulse>::from_u128(13).into(),
                    first_input.into(),
                    DiagnosticMeta::default(),
                ),
                ConnectionDef::new(
                    ConnectionKey::from_u128(24),
                    ExternalInputKey::<Pulse>::from_u128(23).into(),
                    second_input.into(),
                    DiagnosticMeta::default(),
                ),
            ],
        );
        let compiled = compile(network);
        let mut machine = compiled.spawn(generous_policy());
        let snapshot = match compiled
            .input_snapshot()
            .pulse(ExternalInputKey::<Pulse>::from_u128(13), PulseCount::ONE)
            .and_then(|builder| {
                builder.pulse(ExternalInputKey::<Pulse>::from_u128(23), PulseCount::ONE)
            })
            .and_then(crate::InputSnapshotBuilder::finish)
        {
            Ok(snapshot) => snapshot,
            Err(failure) => panic!("fixture snapshot must bind: {failure}"),
        };
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(1),
                machine.revision(),
                snapshot,
            ))
            .unwrap_or_else(|failure| panic!("fixture initialization must commit: {failure}"));
        let before_keys = machine.pending_keys_in_storage_order();
        assert!(before_keys.len() >= 2);
        let execution = execution_digest_input(&machine, 3, 3);
        let observable = observable_digest_input(&machine, 3, 3);
        machine.reverse_pending_batches_for_test();
        assert_ne!(machine.pending_keys_in_storage_order(), before_keys);
        assert_eq!(execution_digest_input(&machine, 3, 3), execution);
        assert_eq!(observable_digest_input(&machine, 3, 3), observable);
    }

    fn pulse_delay_node(node: u128, input: InPortKey<Pulse>, output: u128) -> NodeDef<()> {
        NodeDef::new(
            NodeKey::from_u128(node),
            NodeKind::pulse_delay(span(4)),
            NodePorts::with_input_roles(
                vec![input.into()],
                vec![InputPortRole::PulseDelay],
                vec![OutPortKey::<Pulse>::from_u128(output).into()],
            ),
            DiagnosticMeta::default(),
        )
    }

    #[test]
    fn supporter_order_and_episode_collection_order_are_not_semantic() {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let high = builder.constant(LogicLevel::High);
        builder
            .add_level_set_reset_latch(
                NodeKey::from_u128(3),
                high,
                high,
                LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("fixture latch must author: {failure:?}"));
        builder
            .add_level_set_reset_latch(
                NodeKey::from_u128(4),
                high,
                high,
                LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("fixture latch must author: {failure:?}"));
        let compiled = compile_builder(builder);
        let mut machine = compiled.spawn(generous_policy());
        initialize(&mut machine, 1);
        let ordinals = machine.supporter_ordinals_for_test();
        assert!(ordinals.len() >= 2);
        let execution = execution_digest_input(&machine, 3, 3);
        let observable = observable_digest_input(&machine, 3, 3);
        machine.reverse_provenance_supporters_for_test();
        assert_ne!(machine.supporter_ordinals_for_test(), ordinals);
        assert_eq!(execution_digest_input(&machine, 3, 3), execution);
        assert_eq!(observable_digest_input(&machine, 3, 3), observable);

        let identities: Vec<_> = machine
            .store
            .active_episodes
            .values()
            .map(|episode| episode.identity().as_bytes())
            .collect();
        assert!(identities.len() >= 2);
        let mut reversed = identities.clone();
        reversed.reverse();
        assert_ne!(identities, reversed);
        let mut sorted = identities.clone();
        sorted.sort();
        let mut resorted = reversed.clone();
        resorted.sort();
        assert_eq!(sorted, resorted);
        machine.rebuild_episodes_reversed_for_test();
        assert_eq!(execution_digest_input(&machine, 3, 3), execution);
        let bytes = execution_digest_input(&machine, 3, 3);
        let first = bytes.windows(32).position(|window| window == sorted[0]);
        let second = bytes.windows(32).position(|window| window == sorted[1]);
        match (first, second) {
            (Some(first), Some(second)) => assert!(first < second),
            _ => panic!("episode identity bytes must appear in canonical order"),
        }
    }

    #[test]
    fn module_owned_state_uses_the_instance_and_local_key() {
        let local_node = NodeKey::from_u128(0x22);
        let local_input = InPortKey::<Pulse>::from_u128(0x23);
        let local_output = OutPortKey::<Level>::from_u128(0x24);
        let module_input = ModuleInputKey::<Pulse>::from_u128(0x31);
        let module_output = ModuleOutputKey::<Level>::from_u128(0x32);
        let module = UncheckedModule::new_user(
            DiagnosticMeta::default(),
            vec![ModuleInputDef::new(
                module_input.into(),
                DiagnosticMeta::default(),
            )],
            vec![ModuleOutputDef::new(
                module_output.into(),
                DiagnosticMeta::default(),
            )],
            vec![
                ModuleInterfaceMapping::input(module_input.into(), local_input.into()),
                ModuleInterfaceMapping::output(module_output.into(), local_output.into()),
            ],
            vec![NodeDef::new(
                local_node,
                NodeKind::toggle(LogicLevel::High),
                NodePorts::with_input_roles(
                    vec![local_input.into()],
                    vec![InputPortRole::Toggle],
                    vec![local_output.into()],
                ),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
        );
        let module = match module.validate_ref().artifact() {
            Some(module) => module.clone(),
            None => panic!("fixture module must validate"),
        };
        let instance = ModuleInstanceKey::from_u128(0x41);
        let external = ExternalInputKey::<Pulse>::from_u128(0x51);
        let network = UncheckedNetwork::new_with_instances(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            Vec::new(),
            vec![ExternalInputDef::new(
                external.into(),
                DiagnosticMeta::default(),
            )],
            vec![ExternalOutputDef::new(
                ExternalOutputKey::<Level>::from_u128(0x52).into(),
                SignalSourceKey::ModuleOutput {
                    instance,
                    output: module_output,
                }
                .into(),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            vec![ModuleInstanceDef::new(
                instance,
                module,
                ModuleBindingSet::new(vec![ModuleBinding::new(
                    module_input.into(),
                    ConnectionEndpoint::external_input(external.into()),
                )]),
                None,
                DiagnosticMeta::default(),
            )],
        );
        let compiled = compile(network);
        let machine = compiled.clone().spawn(generous_policy());
        let bytes = execution_digest_input(&machine, 3, 3);
        assert!(
            bytes
                .windows(b"module_node".len())
                .any(|window| window == b"module_node")
        );
        assert!(
            bytes
                .windows(key_image(instance.as_u128()).len())
                .any(|window| window == key_image(instance.as_u128()))
        );
        assert!(
            bytes
                .windows(key_image(local_node.as_u128()).len())
                .any(|window| window == key_image(local_node.as_u128()))
        );
        let (flat_nodes, _inputs, flat_outputs) = compiled.module_flat_keys();
        for flat in flat_nodes.into_iter().chain(flat_outputs) {
            if flat == local_node.as_u128() || flat == local_output.as_u128() {
                continue;
            }
            assert!(
                !bytes
                    .windows(key_image(flat).len())
                    .any(|window| window == key_image(flat)),
                "flat key {flat:x} must not enter the execution digest"
            );
        }
        let mut ready = compiled.spawn(generous_policy());
        initialize(&mut ready, 1);
        let observable = observable_digest_input(&ready, 1, 1);
        assert!(
            observable
                .windows(b"module_node_output".len())
                .any(|window| window == b"module_node_output")
        );
        assert!(
            observable
                .windows(key_image(local_output.as_u128()).len())
                .any(|window| window == key_image(local_output.as_u128()))
        );
    }

    #[test]
    fn nested_module_state_keeps_distinct_instance_paths() {
        let mut inner = ModuleBuilder::new();
        let (inner_pulse, pulse) = inner.pulse_input("toggle");
        let level = inner
            .toggle(pulse, ToggleConfig::new(LogicLevel::Low))
            .unwrap_or_else(|failure| panic!("inner toggle must author: {failure:?}"));
        let inner_level = inner
            .level_output("level", level)
            .unwrap_or_else(|failure| panic!("inner output must author: {failure:?}"));
        let inner = match inner.finish().require_artifact() {
            Ok(module) => module,
            Err(_) => panic!("inner module must validate"),
        };
        let nested = ModuleInstanceKey::from_u128(0x50);
        let mut outer = ModuleBuilder::new();
        let (outer_pulse, pulse) = outer.pulse_input("toggle");
        let added = outer
            .instantiate(&inner, nested, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("nested instance must author: {failure:?}"))
            .bind_pulse(inner_pulse, pulse)
            .unwrap_or_else(|failure| panic!("nested binding must author: {failure:?}"))
            .finish()
            .unwrap_or_else(|failure| panic!("nested instance must finish: {failure:?}"));
        let outer_level = outer
            .level_output(
                "level",
                added
                    .level_output(inner_level)
                    .unwrap_or_else(|failure| panic!("nested output must resolve: {failure:?}")),
            )
            .unwrap_or_else(|failure| panic!("outer output must author: {failure:?}"));
        let outer = match outer.finish().require_artifact() {
            Ok(module) => module,
            Err(_) => panic!("outer module must validate"),
        };
        let left_instance = ModuleInstanceKey::from_u128(0x61);
        let right_instance = ModuleInstanceKey::from_u128(0x62);
        let left_pulse = ExternalInputKey::<Pulse>::from_u128(0x71);
        let right_pulse = ExternalInputKey::<Pulse>::from_u128(0x72);
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let left_signal = builder
            .add_pulse_input(left_pulse, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("left pulse must author: {failure:?}"));
        let right_signal = builder
            .add_pulse_input(right_pulse, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("right pulse must author: {failure:?}"));
        let left_added = builder
            .instantiate(&outer, left_instance, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("left instance must author: {failure:?}"))
            .bind_pulse(outer_pulse, left_signal)
            .unwrap_or_else(|failure| panic!("left binding must author: {failure:?}"))
            .finish()
            .unwrap_or_else(|failure| panic!("left instance must finish: {failure:?}"));
        let right_added = builder
            .instantiate(&outer, right_instance, DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("right instance must author: {failure:?}"))
            .bind_pulse(outer_pulse, right_signal)
            .unwrap_or_else(|failure| panic!("right binding must author: {failure:?}"))
            .finish()
            .unwrap_or_else(|failure| panic!("right instance must finish: {failure:?}"));
        builder
            .add_level_output(
                ExternalOutputKey::<Level>::from_u128(0x81),
                left_added
                    .level_output(outer_level)
                    .unwrap_or_else(|failure| panic!("left output must resolve: {failure:?}")),
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("left output must author: {failure:?}"));
        builder
            .add_level_output(
                ExternalOutputKey::<Level>::from_u128(0x82),
                right_added
                    .level_output(outer_level)
                    .unwrap_or_else(|failure| panic!("right output must resolve: {failure:?}")),
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("right output must author: {failure:?}"));
        let compiled = compile_builder(builder);
        let mut left = compiled.spawn(generous_policy());
        let mut right = compiled.spawn(generous_policy());
        let initialize = |machine: &mut Machine<()>| {
            let snapshot = match compiled.input_snapshot().finish() {
                Ok(snapshot) => snapshot,
                Err(failure) => panic!("fixture snapshot must bind: {failure}"),
            };
            machine
                .apply(Transaction::initialize(
                    Time::from_ticks(1),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap_or_else(|failure| panic!("fixture initialization must commit: {failure}"));
        };
        initialize(&mut left);
        initialize(&mut right);
        let advance = |machine: &mut Machine<()>, pulse: ExternalInputKey<Pulse>| {
            let delta = match compiled
                .input_delta()
                .pulse(pulse, PulseCount::ONE)
                .and_then(crate::InputDeltaBuilder::finish)
            {
                Ok(delta) => delta,
                Err(failure) => panic!("fixture delta must bind: {failure}"),
            };
            machine
                .apply(Transaction::advance(
                    Time::from_ticks(2),
                    machine.revision(),
                    delta,
                ))
                .unwrap_or_else(|failure| panic!("fixture advance must commit: {failure}"));
        };
        advance(&mut left, left_pulse);
        advance(&mut right, right_pulse);
        let left_bytes = execution_digest_input(&left, 1, 1);
        let right_bytes = execution_digest_input(&right, 1, 1);
        assert_ne!(
            left.execution_state_digest(),
            right.execution_state_digest(),
            "flipping different nested toggles must change execution identity"
        );
        for instance in [left_instance, right_instance, nested] {
            let image = key_image(instance.as_u128());
            assert!(
                left_bytes
                    .windows(image.len())
                    .any(|window| window == image),
                "instance {:x} must enter the execution projection",
                instance.as_u128()
            );
            assert!(
                right_bytes
                    .windows(image.len())
                    .any(|window| window == image),
                "instance {:x} must enter the execution projection",
                instance.as_u128()
            );
        }
    }

    #[test]
    fn generated_mixed_graphs_and_histories_refine_the_uncached_encoder() {
        use crate::{
            FirstEmissionPolicy, InertialDelayConfig, PeriodicConfig, PulseDelayConfig,
            ReenablePhasePolicy, SampleHoldConfig, TransportDelayConfig,
        };
        for seed in 0_u128..9 {
            let mut builder = NetworkBuilder::<()>::with_key(
                NetworkKey::from_u128(1000 + seed),
                crate::TimeDomainId::from_u128(2),
            );
            let input = ExternalInputKey::from_u128(10);
            let trip = ExternalInputKey::from_u128(11);
            let value = builder
                .add_level_input(input, DiagnosticMeta::default())
                .unwrap();
            let pulse = builder
                .add_pulse_input(trip, DiagnosticMeta::default())
                .unwrap();
            let delayed = builder
                .add_pulse_delay(
                    NodeKey::from_u128(100),
                    pulse,
                    PulseDelayConfig::new(span(2)),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let timer = builder
                .add_periodic(
                    NodeKey::from_u128(101),
                    value,
                    PeriodicConfig::new(
                        span(2 + seed as u64 % 3),
                        FirstEmissionPolicy::AfterFirstPeriod,
                        ReenablePhasePolicy::PreservePhase,
                    ),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let merged = builder
                .add_merge(
                    NodeKey::from_u128(102),
                    [pulse, delayed, timer],
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let toggled = builder
                .add_toggle(
                    NodeKey::from_u128(103),
                    merged,
                    ToggleConfig::new(LogicLevel::Low),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let held = builder
                .add_sample_hold(
                    NodeKey::from_u128(104),
                    value,
                    delayed,
                    SampleHoldConfig::new(LogicLevel::Low),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let transported = builder
                .add_transport_delay(
                    NodeKey::from_u128(105),
                    value,
                    TransportDelayConfig::new(span(2), LogicLevel::Low),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let inertial = builder
                .add_inertial_delay(
                    NodeKey::from_u128(106),
                    value,
                    InertialDelayConfig::new(span(3), LogicLevel::Low),
                    DiagnosticMeta::default(),
                )
                .unwrap()
                .into_outputs();
            let instance = ModuleInstanceKey::from_u128(200);
            let standard = match seed % 3 {
                0 => builder
                    .add_pulse_resettable_toggle(
                        instance,
                        merged,
                        delayed,
                        LogicLevel::Low,
                        DiagnosticMeta::default(),
                    )
                    .unwrap()
                    .into_outputs(),
                1 => builder
                    .add_level_resettable_toggle(
                        instance,
                        merged,
                        value,
                        LogicLevel::Low,
                        DiagnosticMeta::default(),
                    )
                    .unwrap()
                    .into_outputs(),
                _ => builder
                    .add_level_resettable_sample_hold(
                        instance,
                        toggled,
                        delayed,
                        value,
                        LogicLevel::Low,
                        LogicLevel::High,
                        DiagnosticMeta::default(),
                    )
                    .unwrap()
                    .into_outputs(),
            };
            let mut chain = value;
            for _ in 0..seed % 4 {
                chain = builder.not(chain).unwrap();
            }
            for (index, signal) in [chain, toggled, held, transported, inertial, standard]
                .into_iter()
                .enumerate()
            {
                builder
                    .add_level_output(
                        ExternalOutputKey::from_u128(300 + index as u128),
                        signal,
                        DiagnosticMeta::default(),
                    )
                    .unwrap();
            }
            let compiled = compile_builder(builder);
            let mut machine = compiled.spawn(generous_policy());
            let snapshot = compiled
                .input_snapshot()
                .set(input, LogicLevel::High)
                .unwrap()
                .pulse(trip, PulseCount::ONE)
                .unwrap()
                .finish()
                .unwrap();
            machine
                .apply(Transaction::initialize(
                    Time::from_ticks(0),
                    machine.revision(),
                    snapshot,
                ))
                .unwrap();
            let owned = machine.inspect_module(instance).unwrap();
            for (step, at) in [1, 2, 2, 5, 7, 11].into_iter().enumerate() {
                let value = if (seed + step as u128) % 3 == 0 {
                    LogicLevel::Low
                } else {
                    LogicLevel::High
                };
                let delta = compiled
                    .input_delta()
                    .set(input, value)
                    .unwrap()
                    .pulse(trip, PulseCount::new((seed as u64 + step as u64) % 4))
                    .unwrap()
                    .finish()
                    .unwrap();
                let transaction =
                    Transaction::advance(Time::from_ticks(at), machine.revision(), delta);
                let forecast = machine.forecast(transaction.clone()).unwrap();
                machine.apply(transaction).unwrap();
                assert_eq!(machine.snapshot(), forecast.state().snapshot());
                for event in machine.inspect_pending_events().unwrap() {
                    event.provenance().explain_cause(event.cause).unwrap();
                }
            }
            let source = machine.compiled().graph().external_outputs()[0].source();
            let prepared = machine
                .prepare_patch(
                    machine
                        .patch()
                        .add_external_output(ExternalOutputDef::new(
                            ExternalOutputKey::<Level>::from_u128(400).into(),
                            source,
                            DiagnosticMeta::default(),
                        ))
                        .unwrap()
                        .finish(),
                )
                .require_artifact()
                .unwrap();
            let delta = prepared
                .resulting_compiled()
                .input_delta()
                .finish()
                .unwrap();
            machine
                .apply(
                    Transaction::advance(Time::from_ticks(12), machine.revision(), delta)
                        .with_patch(prepared, crate::ReconfigurationPolicy::RejectStateLoss)
                        .unwrap(),
                )
                .unwrap();
            machine.inspect_module(instance).unwrap();
            let snapshot = machine.snapshot();
            let restored = machine
                .compiled()
                .restore(snapshot.clone(), generous_policy())
                .unwrap();
            assert_eq!(snapshot, restored.snapshot());
            drop(machine);
            for (_, cause) in &owned.stateful_standard().unwrap().internal_causes {
                owned.provenance().explain_cause(*cause).unwrap();
            }
        }
    }

    #[test]
    fn version_three_migrated_graph_inputs_and_snapshot_match_goldens() {
        let mut machine = spawn(
            golden_toggle(DiagnosticMeta::default()),
            [100, 10_000, 100, 100, 10_000],
        );
        initialize(&mut machine, 0);
        let delta = machine
            .compiled()
            .input_delta()
            .pulse(ExternalInputKey::from_u128(6), PulseCount::new(2))
            .unwrap()
            .finish()
            .unwrap();
        machine
            .apply(Transaction::advance(
                Time::from_ticks(2),
                machine.revision(),
                delta,
            ))
            .unwrap();
        let source = machine.compiled().graph().external_outputs()[0].source();
        let prepared = machine
            .prepare_patch(
                machine
                    .patch()
                    .add_external_output(ExternalOutputDef::new(
                        ExternalOutputKey::<Level>::from_u128(99).into(),
                        source,
                        DiagnosticMeta::default(),
                    ))
                    .unwrap()
                    .finish(),
            )
            .require_artifact()
            .unwrap();
        let delta = prepared
            .resulting_compiled()
            .input_delta()
            .finish()
            .unwrap();
        machine
            .apply(
                Transaction::advance(Time::from_ticks(3), machine.revision(), delta)
                    .with_patch(prepared, crate::ReconfigurationPolicy::RejectStateLoss)
                    .unwrap(),
            )
            .unwrap();
        crate::state_digest_reference::assert_machine(&machine);
        let snapshot = machine.snapshot();
        for (name, bytes) in [
            (
                "execution_state_migrated_v3.hex",
                execution_digest_input(&machine, 3, 3),
            ),
            (
                "observable_state_migrated_v3.hex",
                observable_digest_input(&machine, 3, 3),
            ),
            (
                "machine_snapshot_migrated_v3.hex",
                snapshot.artifact_bytes().to_vec(),
            ),
        ] {
            assert_golden(name, &bytes);
        }
    }

    #[test]
    fn differing_records_with_one_digest_are_fatal() {
        let mut table = ContentTable::default();
        table.insert([9; 32], Arc::new(vec![1, 2, 3]));
        table.insert([9; 32], Arc::new(vec![1, 2, 3]));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            table.insert([9; 32], Arc::new(vec![4]));
        }));
        assert!(result.is_err());
    }
}
