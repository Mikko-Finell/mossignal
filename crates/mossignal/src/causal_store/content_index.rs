//! Graph-local canonical consistency; immutable paths share without old-table copies.
use super::CanonicalRecord;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(super) struct ContentIndex {
    root: Option<Arc<Entry>>,
}

struct Entry {
    value: Value,
    #[cfg(test)]
    tracker: Arc<std::sync::atomic::AtomicUsize>,
}

enum Value {
    Leaf {
        digest: [u8; 32],
        payload: Arc<Vec<u8>>,
    },
    Branch {
        bit: usize,
        left: Arc<Entry>,
        right: Arc<Entry>,
    },
}

impl Entry {
    fn new(value: Value) -> Arc<Self> {
        #[cfg(test)]
        crate::causal_work::update(|w| {
            if crate::causal_work::is_selecting_content() {
                w.content_selection.nodes_created += 1;
                w.content_selection.leaves_created +=
                    usize::from(matches!(&value, Value::Leaf { .. }));
            }
            if crate::causal_work::is_freezing() {
                w.freeze.consistency_nodes_created += 1;
            } else {
                w.membership_content_nodes_created += 1;
            }
        });
        Arc::new(Self {
            value,
            #[cfg(test)]
            tracker: crate::causal_work::content_tracker(),
        })
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        #[cfg(test)]
        self.tracker
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        #[cfg(test)]
        crate::causal_work::update(|w| w.content_nodes_released += 1);
        // Branch bits strictly increase: both traversal and release depth are at most 256.
    }
}

fn right(digest: &[u8; 32], bit: usize) -> bool {
    digest[bit / 8] & (0x80 >> (bit % 8)) != 0
}

fn visit() {
    #[cfg(test)]
    crate::causal_work::update(|w| {
        if crate::causal_work::is_selecting_content() {
            w.content_selection.visits += 1;
        }
        if crate::causal_work::is_freezing() {
            w.freeze.consistency_path_visits += 1;
        } else {
            w.membership_content_visits += 1;
        }
    });
}

impl ContentIndex {
    // SPEC: docs/specs/contracts/provenance-retention.yaml "independent-artifact-ownership"
    // Share only exact selected content; an excluded leaf must not survive in an old subtree.
    pub(super) fn select(&self, digests: &BTreeSet<[u8; 32]>) -> Self {
        Self {
            root: self
                .root
                .as_ref()
                .and_then(|root| Self::select_entry(root, digests)),
        }
    }

    fn select_entry(entry: &Arc<Entry>, digests: &BTreeSet<[u8; 32]>) -> Option<Arc<Entry>> {
        visit();
        match &entry.value {
            Value::Leaf { digest, .. } => {
                if !digests.contains(digest) {
                    return None;
                }
                #[cfg(test)]
                crate::causal_work::update(|w| w.content_selection.leaves_shared += 1);
                Some(Arc::clone(entry))
            }
            Value::Branch { bit, left, right } => {
                match (
                    Self::select_entry(left, digests),
                    Self::select_entry(right, digests),
                ) {
                    (None, None) => None,
                    (Some(child), None) | (None, Some(child)) => {
                        #[cfg(test)]
                        crate::causal_work::update(|w| w.content_selection.branches_promoted += 1);
                        Some(child)
                    }
                    (Some(selected_left), Some(selected_right)) => {
                        if Arc::ptr_eq(left, &selected_left) && Arc::ptr_eq(right, &selected_right)
                        {
                            #[cfg(test)]
                            crate::causal_work::update(|w| {
                                w.content_selection.branches_shared += 1
                            });
                            Some(Arc::clone(entry))
                        } else {
                            Some(Entry::new(Value::Branch {
                                bit: *bit,
                                left: selected_left,
                                right: selected_right,
                            }))
                        }
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn contents_for_test(&self) -> std::collections::BTreeMap<[u8; 32], Arc<Vec<u8>>> {
        let mut contents = std::collections::BTreeMap::new();
        let mut pending: Vec<_> = self.root.iter().collect();
        while let Some(entry) = pending.pop() {
            match &entry.value {
                Value::Leaf { digest, payload } => {
                    assert!(contents.insert(*digest, Arc::clone(payload)).is_none());
                }
                Value::Branch { left, right, .. } => pending.extend([left, right]),
            }
        }
        contents
    }

    pub(super) fn insert(&mut self, content: &CanonicalRecord) {
        #[cfg(test)]
        if crate::causal_work::is_selecting_content() {
            crate::causal_work::update(|w| w.content_selection.insertions += 1);
        }
        #[cfg(test)]
        if crate::causal_work::is_freezing() {
            crate::causal_work::update(|w| w.freeze.consistency_lookups += 1);
        }
        let Some(root) = &self.root else {
            self.root = Some(Entry::new(Value::Leaf {
                digest: content.digest,
                payload: Arc::clone(&content.payload),
            }));
            return;
        };
        let mut current = root;
        let (old_digest, old_payload) = loop {
            visit();
            match &current.value {
                Value::Leaf { digest, payload } => break (digest, payload),
                Value::Branch {
                    bit,
                    left,
                    right: child,
                } => {
                    current = if right(&content.digest, *bit) {
                        child
                    } else {
                        left
                    };
                }
            }
        };
        if *old_digest == content.digest {
            // SPEC: docs/specs/contracts/machine-state-digests.yaml "equality-is-not-validation"
            // Equal digest with unequal available bytes is fatal, including old/result-only facts.
            if old_payload != &content.payload {
                panic!(
                    "digest collision: two different canonical provenance records share one content digest"
                );
            }
            return;
        }
        let Some((byte, difference)) = old_digest
            .iter()
            .zip(content.digest)
            .enumerate()
            .map(|(i, (old, new))| (i, old ^ new))
            .find(|(_, difference)| *difference != 0)
        else {
            panic!("different content keys must have a differing digest bit");
        };
        let split = byte * 8 + difference.leading_zeros() as usize;
        self.root = Some(Self::insert_at(root, content, split));
    }

    fn insert_at(current: &Arc<Entry>, content: &CanonicalRecord, split: usize) -> Arc<Entry> {
        visit();
        if let Value::Branch {
            bit,
            left,
            right: child,
        } = &current.value
        {
            if *bit < split {
                let (left, child) = if right(&content.digest, *bit) {
                    (Arc::clone(left), Self::insert_at(child, content, split))
                } else {
                    (Self::insert_at(left, content, split), Arc::clone(child))
                };
                return Entry::new(Value::Branch {
                    bit: *bit,
                    left,
                    right: child,
                });
            }
        }
        let leaf = Entry::new(Value::Leaf {
            digest: content.digest,
            payload: Arc::clone(&content.payload),
        });
        let (left, child) = if right(&content.digest, split) {
            (Arc::clone(current), leaf)
        } else {
            (leaf, Arc::clone(current))
        };
        Entry::new(Value::Branch {
            bit: split,
            left,
            right: child,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn payload(key: [u8; 32], bytes: Vec<u8>) -> CanonicalRecord {
        CanonicalRecord {
            digest: key,
            payload: Arc::new(bytes),
        }
    }

    fn lookup(index: &ContentIndex, key: [u8; 32]) -> Option<&Arc<Vec<u8>>> {
        let mut node = index.root.as_ref()?;
        loop {
            match &node.value {
                Value::Leaf { digest, payload } => return (*digest == key).then_some(payload),
                Value::Branch {
                    bit,
                    left,
                    right: child,
                } => node = if right(&key, *bit) { child } else { left },
            }
        }
    }

    fn assert_shape(entry: &Entry, path: &mut Vec<(usize, bool)>) {
        match &entry.value {
            Value::Leaf { digest, .. } => {
                for (bit, side) in path.iter() {
                    assert_eq!(right(digest, *bit), *side);
                }
            }
            Value::Branch { bit, left, right } => {
                assert!(*bit < 256);
                assert!(path.last().is_none_or(|(parent, _)| parent < bit));
                path.push((*bit, false));
                assert_shape(left, path);
                path.last_mut().unwrap().1 = true;
                assert_shape(right, path);
                path.pop();
            }
        }
    }

    #[test]
    fn content_selection_filter_matches_flat_map_at_every_split_bit() {
        for reverse in [false, true] {
            let mut source = ContentIndex::default();
            let mut expected = BTreeMap::new();
            let keys: Vec<_> = (0..256)
                .map(|bit| {
                    let mut key = [0; 32];
                    key[bit / 8] = 0x80 >> (bit % 8);
                    key
                })
                .chain(std::iter::once([0; 32]))
                .collect();
            for i in 0..keys.len() {
                let key = keys[if reverse { keys.len() - i - 1 } else { i }];
                let content = payload(key, key.to_vec());
                source.insert(&content);
                expected.insert(key, content.payload);
            }
            for chosen in [
                BTreeSet::new(),
                keys.iter().copied().collect(),
                BTreeSet::from([keys[0], keys[255], keys[256]]),
                keys.iter().step_by(2).copied().collect(),
                keys.iter().skip(1).copied().collect(),
                keys.iter().take(256).copied().collect(),
            ] {
                let oracle: BTreeMap<_, _> = expected
                    .iter()
                    .filter(|(key, _)| chosen.contains(*key))
                    .map(|(key, value)| (*key, Arc::clone(value)))
                    .collect();
                crate::causal_work::reset();
                let selected = {
                    let _scope = crate::causal_work::content_selection_scope();
                    source.select(&chosen)
                };
                let w = crate::causal_work::read();
                assert_eq!(selected.contents_for_test(), oracle);
                assert_eq!(w.content_selection.visits, 2 * keys.len() - 1);
                assert_eq!(w.membership_content_visits, w.content_selection.visits);
                assert_eq!(
                    w.membership_content_nodes_created,
                    w.content_selection.nodes_created
                );
                assert_eq!(w.content_selection.leaves_created, 0);
                assert_eq!(w.content_selection.insertions, 0);
                assert_eq!(w.content_selection.leaves_shared, chosen.len());
                assert!(
                    w.content_selection.nodes_created
                        <= (keys.len() - 1).min(256 * (keys.len() - chosen.len()))
                );
                for (key, value) in &oracle {
                    assert!(Arc::ptr_eq(lookup(&selected, *key).unwrap(), value));
                }
                if let Some(root) = &selected.root {
                    assert_shape(root, &mut Vec::new());
                }
                if chosen.len() == keys.len() {
                    assert!(Arc::ptr_eq(
                        source.root.as_ref().unwrap(),
                        selected.root.as_ref().unwrap()
                    ));
                    assert_eq!(w.content_selection.nodes_created, 0);
                    assert_eq!(w.content_selection.branches_shared, keys.len() - 1);
                }
                if chosen.len() == keys.len() - 1 {
                    assert!(w.content_selection.branches_promoted > 0);
                    if !chosen.contains(&keys[0]) {
                        assert!(w.content_selection.branches_shared > 0);
                    }
                }
            }
            drop(expected);
            drop(source);
            assert_eq!(crate::causal_work::live_content_nodes(), 0);
        }
    }

    #[test]
    fn content_selection_depth_256_final_shared_owner_releases_all_entries() {
        let mut source = ContentIndex::default();
        let zero = payload([0; 32], vec![0]);
        let bytes = Arc::downgrade(&zero.payload);
        source.insert(&zero);
        drop(zero);
        let mut keys = BTreeSet::from([[0; 32]]);
        for bit in 0..256 {
            let mut key = [0; 32];
            key[bit / 8] = 0x80 >> (bit % 8);
            source.insert(&payload(key, key.to_vec()));
            keys.insert(key);
        }
        let root = Arc::downgrade(source.root.as_ref().unwrap());
        let selected = source.select(&keys);
        let other = selected.clone();
        drop(source);
        drop(selected);
        assert!(root.upgrade().is_some() && bytes.upgrade().is_some());
        assert_eq!(crate::causal_work::live_content_nodes(), 513);
        drop(other);
        assert!(root.upgrade().is_none() && bytes.upgrade().is_none());
        assert_eq!(crate::causal_work::live_content_nodes(), 0);
    }

    #[test]
    fn content_selection_shared_subtrees_release_excluded_bytes_and_validate_new_content() {
        let mut source = ContentIndex::default();
        let mut excluded = None;
        for i in 0..16 {
            let key = [i; 32];
            let record = payload(key, vec![i]);
            if i == 15 {
                excluded = Some(Arc::downgrade(&record.payload));
            }
            source.insert(&record);
        }
        let old_root = Arc::downgrade(source.root.as_ref().unwrap());
        let mut selected = source.select(&(0..15).map(|i| [i; 32]).collect());
        let selected_root = Arc::downgrade(selected.root.as_ref().unwrap());
        let other = selected.clone();
        drop(source);
        assert!(old_root.upgrade().is_none());
        assert!(excluded.unwrap().upgrade().is_none());
        for i in 0..15 {
            assert_eq!(lookup(&selected, [i; 32]).unwrap().as_slice(), &[i]);
        }
        selected.insert(&payload([0; 32], vec![0]));
        assert!(Arc::ptr_eq(
            selected.root.as_ref().unwrap(),
            &selected_root.upgrade().unwrap()
        ));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || selected.insert(&payload([0; 32], vec![99]))
            ))
            .is_err()
        );
        assert!(Arc::ptr_eq(
            selected.root.as_ref().unwrap(),
            &selected_root.upgrade().unwrap()
        ));
        // Re-admission of an excluded digest checks newly available bytes normally.
        selected.insert(&payload([15; 32], vec![15]));
        assert!(lookup(&other, [15; 32]).is_none());
        drop(selected);
        assert!(selected_root.upgrade().is_some());
        drop(other);
        assert!(selected_root.upgrade().is_none());
        assert_eq!(crate::causal_work::live_content_nodes(), 0);
    }

    #[test]
    fn incremental_freezing_lookup_matches_flat_map_and_forks_share_unchanged_leaves() {
        // Include every possible split bit, in both insertion orders.
        for reverse in [false, true] {
            let mut index = ContentIndex::default();
            let mut expected = BTreeMap::new();
            for i in 0..256 {
                let bit = if reverse { 255 - i } else { i };
                let mut key = [0; 32];
                key[bit / 8] = 0x80 >> (bit % 8);
                let content = payload(key, key.to_vec());
                index.insert(&content);
                expected.insert(key, content.payload);
            }
            let zero = payload([0; 32], vec![0]);
            index.insert(&zero);
            expected.insert(zero.digest, zero.payload);
            let mut fork = index.clone();
            let key = [0xff; 32];
            fork.insert(&payload(key, vec![7]));
            assert!(lookup(&index, key).is_none());
            for (key, bytes) in &expected {
                assert_eq!(lookup(&index, *key), Some(bytes));
                assert!(Arc::ptr_eq(
                    lookup(&index, *key).unwrap(),
                    lookup(&fork, *key).unwrap()
                ));
            }
            drop(index);
            drop(fork);
            assert_eq!(crate::causal_work::live_content_nodes(), 0);
        }
    }

    #[test]
    fn incremental_freezing_new_and_shared_old_collisions_are_fatal_without_overwrite() {
        let original = payload([9; 32], vec![1, 2, 3]);
        let mut index = ContentIndex::default();
        index.insert(&original);
        let root = Arc::clone(index.root.as_ref().unwrap());
        index.insert(&payload([9; 32], vec![1, 2, 3]));
        assert!(Arc::ptr_eq(&root, index.root.as_ref().unwrap()));
        let mut fork = index.clone();
        for candidate in [&mut index, &mut fork] {
            let before = Arc::clone(candidate.root.as_ref().unwrap());
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || candidate.insert(&payload([9; 32], vec![4]))
                ))
                .is_err()
            );
            assert!(Arc::ptr_eq(&before, candidate.root.as_ref().unwrap()));
            assert_eq!(lookup(candidate, [9; 32]), Some(&original.payload));
        }
        drop(root);
        drop(index);
        drop(fork);
        assert_eq!(crate::causal_work::live_content_nodes(), 0);
    }
}
