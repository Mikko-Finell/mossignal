//! Allocation-free preflight of the existing committed-state projection.
//!
//! This counts source views (including duplicate views), not just deduplicated
//! artifact records: indexing scratch must be admitted before projection starts.
use crate::transaction::ProvenanceRecord;
use crate::{Machine, ProvenanceView};
use core::fmt;

/// Explicit resource ceilings for exporting one committed machine.
#[derive(Clone, Copy, Debug)]
pub struct SnapshotLimits {
    /// Maximum source causal records visited, including separate episode views.
    pub records: u64,
    /// Maximum predecessor entries visited, including pulse contributions.
    pub edges: u64,
    /// Maximum conservative encoded artifact size.
    pub artifact_bytes: u64,
    /// Maximum conservative additional live allocation, including the result.
    pub allocation_bytes: u64,
}

/// Conservative reservation for a persistence operation, not an RSS measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotAllocation {
    /// Upper bound on additional simultaneously live heap storage.
    pub bytes: u64,
}

/// A persistence operation exceeded a caller-owned resource allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnapshotResourceFailure {
    /// Checked resource arithmetic could not be represented.
    Overflow,
    /// A named allowance is insufficient; no projection was started.
    Limit {
        /// Resource being reserved.
        resource: &'static str,
        /// Caller allowance.
        limit: u64,
        /// Conservative required amount.
        required: u64,
    },
}
impl fmt::Display for SnapshotResourceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => f.write_str("snapshot resource accounting overflow"),
            Self::Limit {
                resource,
                limit,
                required,
            } => write!(
                f,
                "snapshot {resource} requires {required}, allowance is {limit}"
            ),
        }
    }
}
impl std::error::Error for SnapshotResourceFailure {}
pub(crate) type Result<T> = core::result::Result<T, SnapshotResourceFailure>;
pub(crate) fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(SnapshotResourceFailure::Overflow)
}
pub(crate) fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or(SnapshotResourceFailure::Overflow)
}
pub(crate) fn check(resource: &'static str, required: u64, limit: u64) -> Result<()> {
    if required > limit {
        Err(SnapshotResourceFailure::Limit {
            resource,
            limit,
            required,
        })
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct Causes {
    records: u64,
    edges: u64,
    payload: u64,
}
impl Causes {
    fn include<D>(&mut self, view: &ProvenanceView<D>) -> Result<()> {
        self.records = add(self.records, view.records().len() as u64)?;
        for (index, record) in view.records().iter().enumerate() {
            let contributions = match record {
                ProvenanceRecord::PulseDerived { contributions, .. }
                | ProvenanceRecord::PulseControlledLevel { contributions, .. } => {
                    contributions.len()
                }
                _ => 0,
            };
            self.edges = add(
                self.edges,
                add(record.supporters().len() as u64, contributions as u64)?,
            )?;
            let Some(content) = view.records().canonical(index).get() else {
                panic!("snapshot preflight requires frozen committed causal content");
            };
            self.payload = add(self.payload, content.payload.len() as u64)?;
        }
        Ok(())
    }
}

pub(crate) fn export<D>(
    machine: &Machine<D>,
    limits: SnapshotLimits,
) -> Result<SnapshotAllocation> {
    let mut causes = Causes::default();
    if let Some(view) = &machine.store.provenance {
        causes.include(view)?;
    }
    for episode in machine.store.active_episodes.values() {
        causes.include(episode.provenance())?;
    }
    check("records", causes.records, limits.records)?;
    check("edges", causes.edges, limits.edges)?;
    let topology = machine.compiled.persistence_shape()?;
    let store = &machine.store;
    let mut entries = topology.entries;
    for count in [
        store.operation_causes.len(),
        store.external_levels.len(),
        store.output_baselines.len(),
        store.input_causes.len(),
        store.output_causes.len(),
        store.edge_observation_causes.len(),
        store.toggle_inversion_causes.len(),
        store.establishment_causes.len(),
        store.transport_transition_causes.len(),
        store.inertial_cancellation_causes.len(),
        store.periodic_anchor_causes.len(),
        store.periodic_cancellation_causes.len(),
        store.active_episodes.len(),
    ] {
        entries = add(entries, count as u64)?;
    }
    entries = add(entries, mul(store.standard_causes.len() as u64, 3)?)?;
    for batch in store.pending_events.values() {
        entries = add(entries, batch.len() as u64)?;
    }
    // Every non-causal row has fewer than 32 fixed fields of at most 32 bytes,
    // plus one qualified path (17 encoded bytes/key). A source entry can appear
    // in both state tables and in the current/root lists; four rows cover these.
    // The fixed envelope/payload/version records fit in 4096 bytes. Frozen
    // causal payloads already contain their full historical subjects and edges.
    let row = add(1024, mul(topology.path, 17)?)?;
    let encoded = add(4096, add(causes.payload, mul(mul(entries, 4)?, row)?)?)?;
    check("artifact_bytes", encoded, limits.artifact_bytes)?;
    // Payload, bare/full envelopes, digest input and standalone bytes overlap.
    // 16 copies include Vec growth and nested row/Record buffers. Index maps,
    // sets, root vectors, sort workspace, traversal worklists and owner clones
    // are covered separately (4096/entry exceeds their combined node layouts).
    let indices = mul(add(add(causes.records, causes.edges)?, entries)?, 4096)?;
    let bytes = add(mul(encoded, 16)?, indices)?;
    check(
        "allocation_bytes",
        bytes,
        limits.allocation_bytes.min(isize::MAX as u64),
    )?;
    Ok(SnapshotAllocation { bytes })
}
