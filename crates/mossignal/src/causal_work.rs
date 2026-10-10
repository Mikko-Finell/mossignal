//! Test-only accounting at the actual causal construction/encoding boundaries.

use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    StateExtraction,
    Provenance,
    DigestPreparation,
    Result,
    Projection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ObsoleteRemaps {
    pub same_lineage_attempts: usize,
    pub pending_visits: usize,
    pub output_visits: usize,
    pub diagnostic_visits: usize,
    pub cause_map_visits: usize,
    pub carried_map_visits: usize,
    pub read_only_map_visits: usize,
    pub standard_role_visits: usize,
    pub new_record_visits: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FreezeWork {
    pub old_records_indexed: usize,
    pub scratch_slots: usize,
    pub cached_payload_visits: usize,
    pub cached_payload_insertions: usize,
    pub old_source_visits: usize,
    pub new_nodes: usize,
    pub predecessor_reads: usize,
    pub consistency_lookups: usize,
    pub consistency_path_visits: usize,
    pub consistency_nodes_created: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    pub freeze: FreezeWork,
    pub membership_content_visits: usize,
    pub membership_content_nodes_created: usize,
    pub content_nodes_released: usize,
    pub obsolete_remaps: ObsoleteRemaps,
    pub different_lineage_remaps: usize,
    pub source_translation_lookups: usize,
    pub successor_cause_entries_copied: usize,
    pub predecessors_owned: usize,
    pub nodes_created: usize,
    pub nodes_released: usize,
    pub logical_growth: u64,
    pub old_record_rewrites: usize,
    pub scope_records_hashed: usize,
    pub canonical_encodes: usize,
    pub canonical_hashes: usize,
    pub namespace_allocations: usize,
    pub catalogue_handles_copied: usize,
    pub membership_entries_copied: usize,
    pub owned_closure_records_visited: usize,
    pub closure_records_visited: usize,
    pub closure_records_emitted: usize,
    pub closure_bytes_emitted: usize,
}

thread_local! {
    static WORK: Cell<Work> = Cell::new(Work::default());
    static LIVE: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    static LIVE_CONTENT: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    static FAULT: Cell<Option<Fault>> = const { Cell::new(None) };
    static FREEZING: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn is_freezing() -> bool {
    FREEZING.with(Cell::get)
}

pub(crate) fn with_freezing<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FREEZING.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(FREEZING.with(|cell| cell.replace(true)));
    run()
}

pub(crate) fn node_tracker() -> Arc<AtomicUsize> {
    LIVE.with(|tracker| {
        tracker.fetch_add(1, Ordering::Relaxed);
        Arc::clone(tracker)
    })
}

pub(crate) fn content_tracker() -> Arc<AtomicUsize> {
    LIVE_CONTENT.with(|tracker| {
        tracker.fetch_add(1, Ordering::Relaxed);
        Arc::clone(tracker)
    })
}

pub(crate) fn live_content_nodes() -> usize {
    LIVE_CONTENT.with(|tracker| tracker.load(Ordering::Relaxed))
}

pub(crate) fn live_nodes() -> usize {
    LIVE.with(|tracker| tracker.load(Ordering::Relaxed))
}

pub(crate) fn inject(fault: Fault) {
    FAULT.with(|cell| cell.set(Some(fault)));
}

pub(crate) fn take_fault(fault: Fault) -> bool {
    FAULT.with(|cell| {
        if cell.get() == Some(fault) {
            cell.set(None);
            true
        } else {
            false
        }
    })
}

pub(crate) fn update(change: impl FnOnce(&mut Work)) {
    WORK.with(|cell| {
        let mut work = cell.get();
        change(&mut work);
        cell.set(work);
    });
}

pub(crate) fn reset() {
    WORK.with(|cell| cell.set(Work::default()));
}

pub(crate) fn read() -> Work {
    WORK.with(Cell::get)
}
