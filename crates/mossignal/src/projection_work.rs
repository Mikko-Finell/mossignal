//! Test-only counts at actual machine projection, hashing and index operations.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    pub execution_builds: usize,
    pub observable_builds: usize,
    pub execution_hashes: usize,
    pub observable_hashes: usize,
    pub execution_hash_bytes: usize,
    pub observable_hash_bytes: usize,
    pub contexts_built: usize,
    pub records_indexed: usize,
    pub closure_visits: usize,
    pub closure_roots: usize,
    pub closure_pops: usize,
    pub predecessor_entries: usize,
    pub content_inserts: usize,
    pub content_new: usize,
    pub content_equal: usize,
    pub direct_resolutions: usize,
    pub records_emitted: usize,
    pub bytes_emitted: usize,
}

thread_local! {
    static WORK: Cell<Work> = Cell::new(Work::default());
    static ACCOUNT: Cell<bool> = const { Cell::new(true) };
}

pub(crate) fn update(change: impl FnOnce(&mut Work)) {
    if ACCOUNT.with(Cell::get) {
        WORK.with(|cell| {
            let mut work = cell.get();
            change(&mut work);
            cell.set(work);
        });
    }
}

pub(crate) fn accounting() -> bool {
    ACCOUNT.with(Cell::get)
}

pub(crate) fn without_accounting<T>(check: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACCOUNT.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ACCOUNT.with(|cell| cell.replace(false)));
    check()
}

pub(crate) fn reset() {
    WORK.with(|cell| cell.set(Work::default()));
}

pub(crate) fn read() -> Work {
    WORK.with(Cell::get)
}
