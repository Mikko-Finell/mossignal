//! Test-only accounting at the actual causal construction/encoding boundaries.

use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    Provenance,
    Result,
    Projection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
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
    pub closure_records_visited: usize,
    pub closure_records_emitted: usize,
    pub closure_bytes_emitted: usize,
}

thread_local! {
    static WORK: Cell<Work> = Cell::new(Work::default());
    static LIVE: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    static FAULT: Cell<Option<Fault>> = const { Cell::new(None) };
}

pub(crate) fn node_tracker() -> Arc<AtomicUsize> {
    LIVE.with(|tracker| {
        tracker.fetch_add(1, Ordering::Relaxed);
        Arc::clone(tracker)
    })
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
