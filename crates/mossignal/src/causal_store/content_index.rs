//! Graph-local canonical consistency; immutable paths share without old-table copies.
use super::CanonicalRecord;
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
        if crate::causal_work::is_freezing() {
            w.freeze.consistency_path_visits += 1;
        } else {
            w.membership_content_visits += 1;
        }
    });
}

impl ContentIndex {
    pub(super) fn insert(&mut self, content: &CanonicalRecord) {
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
