//! Checked allocation admission around the ordinary decoder and restorer.
use super::*;
use crate::persistence::bounds::{add, check, mul};
use crate::{SnapshotAllocation, SnapshotResourceFailure};

/// Failure of bounded snapshot restoration; semantic failures retain their
/// ordinary structured diagnostic rather than being recategorized as limits.
#[derive(Debug)]
#[non_exhaustive]
pub enum BoundedRestoreFailure<D> {
    /// Caller allocation allowance was insufficient.
    Resource(SnapshotResourceFailure),
    /// Existing artifact or restoration validation rejected the input.
    Restore(Box<RestoreFailure<D>>),
}
impl<D> fmt::Display for BoundedRestoreFailure<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Restore(error) => error.fmt(f),
        }
    }
}
impl<D: fmt::Debug> std::error::Error for BoundedRestoreFailure<D> {}
impl<D> From<SnapshotResourceFailure> for BoundedRestoreFailure<D> {
    fn from(value: SnapshotResourceFailure) -> Self {
        Self::Resource(value)
    }
}
impl<D> From<RestoreFailure<D>> for BoundedRestoreFailure<D> {
    fn from(value: RestoreFailure<D>) -> Self {
        Self::Restore(Box::new(value))
    }
}

impl<D> CompiledNetwork<D> {
    /// Decodes and restores a canonical artifact under one allocation allowance.
    ///
    /// The allowance covers additional decoder, validation, candidate and result
    /// heap storage; caller bytes and the already compiled topology are excluded.
    /// A conservative checked reservation precedes each allocating phase. Returned
    /// accounting is the peak reservation (also a conservative retained bound),
    /// not RSS. The usual decode limits and all compatibility checks still apply.
    /// No candidate is published, input changed, transaction replayed or time
    /// advanced on failure. Optional replay history is not used for restoration.
    pub fn restore_snapshot_bounded(
        &self,
        bytes: &[u8],
        decode: &DecodePolicy,
        policy: RuntimePolicy,
        allocation_bytes: u64,
    ) -> Result<(Machine<D>, SnapshotAllocation), BoundedRestoreFailure<D>> {
        let limit = allocation_bytes.min(isize::MAX as u64);
        // A CBOR item consumes at least one input byte. Value backing (including
        // minimum Vec capacity), iterative parser frames, owned field maps,
        // parsed records, canonical/digest buffers and a checkpoint's temporary
        // parse are bounded together by 512 bytes/input byte + fixed overhead.
        // Checkpoint parsing is iterative: historical depth is not a multiplier
        // of simultaneously live parse trees. No input length drives allocation
        // until the ordinary decoder has checked the actual remaining bytes.
        let decoded = add(65_536, mul(bytes.len() as u64, 512)?)?;
        check("decode_allocation_bytes", decoded, limit)?;
        let artifact = check_artifact(bytes, decode).map_err(RestoreFailure::Decode)?;
        let shape = self.persistence_shape()?;
        // Candidate causal views may overlap for every active episode. Charge
        // the full input allowance per view, including duplicate causal payloads,
        // membership/content maps and graph scratch. Never assume deduplication.
        let views = add(artifact.episodes.len() as u64, 1)?;
        let causal = mul(decoded, views)?;
        // State plans, settlement workspace, regenerated standard modules,
        // owner paths and derived indexes scale with already compiled topology.
        let topology = mul(shape.entries, add(16_384, mul(shape.path, 256)?)?)?;
        let required = add(decoded, add(causal, topology)?)?;
        check("restore_allocation_bytes", required, limit)?;
        let machine = restore_checked(self, artifact, policy)?;
        Ok((machine, SnapshotAllocation { bytes: required }))
    }
}
