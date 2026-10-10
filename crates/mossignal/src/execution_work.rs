//! Test-only counts at actual binding-validation and state-copy boundaries.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    pub(crate) binding_slots_checked: usize,
    pub(crate) staged_stores_cloned: usize,
    pub(crate) working_collections_cloned: usize,
    pub(crate) working_items_cloned: usize,
}

thread_local! {
    static WORK: Cell<Work> = Cell::new(Work::default());
    static CLONE_PREPARATION: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn clone_preparation() -> bool {
    CLONE_PREPARATION.with(Cell::get)
}

pub(crate) fn with_clone_preparation<T>(prepare: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CLONE_PREPARATION.with(|mode| mode.set(self.0));
        }
    }
    let _restore = Restore(CLONE_PREPARATION.with(|mode| mode.replace(true)));
    prepare()
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
