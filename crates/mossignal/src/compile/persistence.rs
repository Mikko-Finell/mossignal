//! Borrowed topology dimensions for persistence allocation preflight.
use super::CompiledNetwork;
use crate::persistence::bounds::{Result, add};
pub(crate) struct PersistenceShape {
    pub entries: u64,
    pub path: u64,
}
impl<D> CompiledNetwork<D> {
    pub(crate) fn persistence_shape(&self) -> Result<PersistenceShape> {
        let inner = &self.inner;
        let mut entries = 0;
        for count in [
            inner.nodes.len(),
            inner.ports.len(),
            inner.external_inputs.len(),
            inner.external_outputs.len(),
            inner.connections.len(),
            inner.operations.len(),
            inner.modules.len(),
            inner.module_inputs.len(),
            inner.module_outputs.len(),
        ] {
            entries = add(entries, count as u64)?;
        }
        for neighbors in inner.predecessors.iter().chain(&inner.successors) {
            entries = add(entries, neighbors.len() as u64)?;
        }
        let path = inner
            .qualified_node_reverse
            .values()
            .map(|node| node.instances().len())
            .chain(inner.modules.keys().map(|module| module.instances().len()))
            .max()
            .unwrap_or(0) as u64;
        Ok(PersistenceShape { entries, path })
    }
}
