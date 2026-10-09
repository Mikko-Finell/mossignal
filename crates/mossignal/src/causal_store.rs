//! Private shared causal nodes and explicit, isolated logical catalogues.

use crate::transaction::{CauseRef, ProvenanceRecord};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Index;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

static NEXT_NAMESPACE: AtomicU64 = AtomicU64::new(1);

/// The first half brands the lineage; the second brands an allocation batch.
/// Neither is a persisted identity or an execution/public-identity cursor.
pub(crate) fn fresh_scope(parent: Option<[u8; 32]>) -> [u8; 32] {
    #[cfg(test)]
    crate::causal_work::update(|work| work.namespace_allocations += 1);
    let serial = match NEXT_NAMESPACE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value.checked_add(1)
    }) {
        Ok(value) => value,
        Err(_) => panic!("private causal namespaces must not wrap and alias retained causes"),
    };
    let mut scope = parent.unwrap_or([0; 32]);
    if parent.is_none() {
        scope[..8].copy_from_slice(&serial.to_be_bytes());
    }
    scope[16..24].copy_from_slice(&serial.to_be_bytes());
    scope
}

pub(crate) fn same_lineage(first: [u8; 32], second: [u8; 32]) -> bool {
    first[..16] == second[..16]
}

pub(crate) struct CanonicalRecord {
    pub(crate) digest: [u8; 32],
    pub(crate) payload: Arc<Vec<u8>>,
}

struct Node<D> {
    cause: CauseRef,
    record: ProvenanceRecord<D>,
    predecessors: Vec<Arc<Node<D>>>,
    canonical: OnceLock<CanonicalRecord>,
    #[cfg(test)]
    source: OnceLock<crate::CompiledNetwork<D>>,
    #[cfg(test)]
    tracker: Arc<std::sync::atomic::AtomicUsize>,
}

impl<D> Drop for Node<D> {
    fn drop(&mut self) {
        #[cfg(test)]
        self.tracker.fetch_sub(1, Ordering::Relaxed);
        // SPEC: docs/specs/contracts/semantic-inspection.yaml "retained-cause-ownership"
        // Final-owner release must work even for a long, still-complete ancestry.
        let mut pending = std::mem::take(&mut self.predecessors);
        while let Some(predecessor) = pending.pop() {
            if let Some(mut owned) = Arc::into_inner(predecessor) {
                pending.append(&mut owned.predecessors);
            }
        }
    }
}

/// An explicit machine-local record set, or an artifact's selected closure.
/// Nodes own predecessors, never an earlier catalogue/view or a global arena.
pub(crate) struct Records<D> {
    scope: [u8; 32],
    nodes: Vec<Arc<Node<D>>>,
    positions: BTreeMap<CauseRef, usize>,
}

impl<D> Clone for Records<D> {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope,
            nodes: self.nodes.clone(),
            positions: self.positions.clone(),
        }
    }
}

impl<D> Records<D> {
    pub(crate) fn new(scope: [u8; 32]) -> Self {
        Self {
            scope,
            nodes: Vec::new(),
            positions: BTreeMap::new(),
        }
    }

    pub(crate) fn fork(&self, scope: [u8; 32]) -> Self {
        #[cfg(test)]
        crate::causal_work::update(|work| {
            work.catalogue_handles_copied += self.nodes.len();
            work.membership_entries_copied += self.positions.len();
        });
        Self {
            scope,
            nodes: self.nodes.clone(),
            positions: self.positions.clone(),
        }
    }

    pub(crate) fn push(&mut self, record: ProvenanceRecord<D>) -> CauseRef {
        let ordinal = match u32::try_from(self.nodes.len()) {
            Ok(value) => value,
            Err(_) => panic!("transaction provenance exceeds the supported reference space"),
        };
        let cause = CauseRef::from_parts(self.scope, ordinal);
        let predecessors = record
            .predecessor_causes()
            .into_iter()
            .map(|cause| {
                let Some(position) = self.positions.get(&cause) else {
                    panic!(
                        "immutable causal predecessors must already belong to the prepared graph"
                    );
                };
                Arc::clone(&self.nodes[*position])
            })
            .collect();
        self.positions.insert(cause, self.nodes.len());
        self.nodes.push(Arc::new(Node {
            cause,
            record,
            predecessors,
            canonical: OnceLock::new(),
            #[cfg(test)]
            source: OnceLock::new(),
            #[cfg(test)]
            tracker: crate::causal_work::node_tracker(),
        }));
        cause
    }

    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub(crate) fn position(&self, cause: CauseRef) -> Option<usize> {
        self.positions.get(&cause).copied()
    }
    pub(crate) fn cause(&self, position: usize) -> CauseRef {
        self.nodes[position].cause
    }
    pub(crate) fn canonical(&self, position: usize) -> &OnceLock<CanonicalRecord> {
        &self.nodes[position].canonical
    }
    #[cfg(test)]
    pub(crate) fn freeze_sources(&self, compiled: &crate::CompiledNetwork<D>) {
        for node in &self.nodes {
            node.source.get_or_init(|| compiled.clone());
        }
    }
    #[cfg(test)]
    pub(crate) fn source(&self, position: usize) -> Option<&crate::CompiledNetwork<D>> {
        self.nodes[position].source.get()
    }
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProvenanceRecord<D>> + DoubleEndedIterator {
        self.nodes.iter().map(|node| &node.record)
    }

    pub(crate) fn owned_closure(&self, roots: &[CauseRef]) -> Self {
        let mut selected = BTreeSet::new();
        let mut pending = Vec::new();
        for cause in roots {
            let Some(position) = self.position(*cause) else {
                panic!("an owned artifact must select only resolvable committed causes");
            };
            pending.push(Arc::clone(&self.nodes[position]));
        }
        while let Some(node) = pending.pop() {
            if selected.insert(node.cause) {
                pending.extend(node.predecessors.iter().map(Arc::clone));
            }
        }
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| selected.contains(&node.cause))
            .map(Arc::clone)
            .collect();
        let positions = nodes
            .iter()
            .enumerate()
            .map(|(position, node)| (node.cause, position))
            .collect();
        Self {
            scope: self.scope,
            nodes,
            positions,
        }
    }

    #[cfg(test)]
    pub(crate) fn shared_node(&self, other: &Self, cause: CauseRef) -> bool {
        match (self.position(cause), other.position(cause)) {
            (Some(first), Some(second)) => Arc::ptr_eq(&self.nodes[first], &other.nodes[second]),
            _ => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn reverse_supporters(&mut self) {
        for node in &mut self.nodes {
            let mut record = crate::transaction::remap_record(&node.record, node.cause.scope);
            record.reverse_unordered_supporters();
            let canonical = OnceLock::new();
            if let Some(content) = node.canonical.get() {
                let _ = canonical.set(CanonicalRecord {
                    digest: content.digest,
                    payload: Arc::clone(&content.payload),
                });
            }
            *node = Arc::new(Node {
                cause: node.cause,
                record,
                predecessors: node.predecessors.clone(),
                canonical,
                source: node.source.clone(),
                tracker: crate::causal_work::node_tracker(),
            });
        }
    }
}

impl<D> Index<usize> for Records<D> {
    type Output = ProvenanceRecord<D>;
    fn index(&self, position: usize) -> &Self::Output {
        &self.nodes[position].record
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_final_owner_release_is_iterative_and_frees_every_node() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let scope = fresh_scope(None);
                let mut records = Records::<()>::new(scope);
                let mut parent = records.push(ProvenanceRecord::ReadyTransaction {
                    at: crate::ReactionStamp::from_parts(crate::time::Time::from_ticks(0), 0),
                    revision: crate::NetworkRevision::from_value(0),
                });
                for _ in 0..30_000 {
                    parent = records.push(ProvenanceRecord::Derived {
                        subject: crate::ProvenanceSubject::Node(crate::key::NodeKey::from_u128(1)),
                        supporters: vec![parent],
                    });
                }
                let owned = records.owned_closure(&[parent]);
                drop(records);
                assert_eq!(crate::causal_work::live_nodes(), 30_001);
                drop(owned);
                assert_eq!(crate::causal_work::live_nodes(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
