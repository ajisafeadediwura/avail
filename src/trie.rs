//! Radix-16 Merkle-Patricia trie.
//!
//! This Substrate/Polkadot-specific radix-16 Merkle-Patricia trie is a data structure that
//! associates keys with values, and that allows efficient verification of the integrity of the
//! data.
//!
//! This data structure is a tree composed of nodes, each node being identified by a key. A key
//! consists in a sequence of 4-bits values called *nibbles*. Example key: `[3, 12, 7, 0]`.
//!
//! Some of these nodes contain a value. These values are inserted by calling [`Trie::insert`].
//!
//! A node A is an *ancestor* of another node B if the key of A is a prefix of the key of B. For
//! example, the node whose key is `[3, 12]` is an ancestor of the node whose key is
//! `[3, 12, 8, 9]`. B is a *descendant* of A.
//!
//! Nodes exist only either if they contain a value, or if their key is the longest shared prefix
//! of two or more nodes that contain a value. For example, if nodes `[7, 2, 9, 11]` and
//! `[7, 2, 14, 8]` contain a value, then node `[7, 2]` also exist, because it is the longest
//! prefix shared between the two.
//!
//! The *Merkle value* of a node is composed, amongst other things, of its associated value and of
//! the Merkle value of its descendants. As such, modifying a node modifies the Merkle value of
//! all its ancestors. Note, however, that modifying a node modifies the Merkle value of *only*
//! its ancestors. As such, the time spent calculating the Merkle value of the root node of a trie
//! mostly depends on the number of modifications that are performed on it, and only a bit on the
//! size of the trie.

use alloc::collections::BTreeMap;
use core::convert::TryFrom as _;
use hashbrown::{hash_map::Entry, HashMap};
use parity_scale_codec::Encode as _;

pub mod calculate_root;

/// Radix-16 Merkle-Patricia trie.
// TODO: probably useless, remove
pub struct Trie {
    /// The entries in the tree.
    ///
    /// Since this is a binary tree, the elements are ordered lexicographically.
    /// Example order: "a", "ab", "ac", "b".
    ///
    /// This list only contains the nodes that have an entry in the storage, and not the nodes
    /// that are branches and don't have a storage entry.
    ///
    /// All the keys have an even number of nibbles.
    entries: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl Trie {
    /// Builds a new empty [`Trie`].
    pub fn new() -> Trie {
        Trie {
            entries: BTreeMap::new(),
        }
    }

    /// Inserts a new entry in the trie.
    pub fn insert(&mut self, key: &[u8], value: impl Into<Vec<u8>>) {
        self.entries.insert(key.into(), value.into());
    }

    /// Removes an entry from the trie.
    pub fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.entries.remove(key)
    }

    /// Returns true if the `Trie` is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes all the elements from the trie.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Calculates the Merkle value of the root node.
    pub fn root_merkle_value(&self) -> [u8; 32] {
        calculate_root::root_merkle_value(&calculate_root::Config {
            get_value: &|key: &[u8]| self.entries.get(key).map(|v| &v[..]),
            prefix_keys: &|prefix: &[u8]| {
                self.entries
                    .range(prefix.to_vec()..) // TODO: this to_vec() is annoying
                    .take_while(|(k, _)| k.starts_with(prefix))
                    .map(|(k, _)| From::from(&k[..]))
                    .collect()
            },
        })
    }
}

// TODO: remove testing private methods once we have better tests
#[cfg(test)]
mod tests {
    use super::{common_prefix, Nibble, Trie, TrieNodeKey};
    use core::iter;

    #[test]
    fn common_prefix_works_trivial() {
        let a = vec![Nibble(0)];

        let obtained = common_prefix([&a[..]].iter().cloned());
        assert_eq!(obtained, Some(a));
    }

    #[test]
    fn common_prefix_works_empty() {
        let obtained = common_prefix(iter::empty());
        assert_eq!(obtained, None);
    }

    #[test]
    fn common_prefix_works_basic() {
        let a = vec![Nibble(5), Nibble(4), Nibble(6)];
        let b = vec![Nibble(5), Nibble(4), Nibble(9), Nibble(12)];

        let obtained = common_prefix([&a[..], &b[..]].iter().cloned());
        assert_eq!(obtained, Some(vec![Nibble(5), Nibble(4)]));
    }

    #[test]
    fn trie_root_one_node() {
        let mut trie = Trie::new();
        trie.insert(b"abcd", b"hello world".to_vec());
        let hash = trie.root_merkle_value();
        // TODO: compare against expected
    }

    #[test]
    fn trie_root_unhashed_empty() {
        let trie = Trie::new();
        let obtained = trie.node_value(
            TrieNodeKey {
                nibbles: Vec::new(),
            },
            None,
            TrieNodeKey {
                nibbles: Vec::new(),
            },
        );
        assert_eq!(obtained, vec![0x0]);
    }

    #[test]
    fn trie_root_unhashed_single_tuple() {
        let mut trie = Trie::new();
        trie.insert(&[0xaa], [0xbb].to_vec());
        let obtained = trie.node_value(
            TrieNodeKey {
                nibbles: Vec::new(),
            },
            None,
            TrieNodeKey::from_bytes(&[0xaa]),
        );

        fn to_compact(n: u8) -> u8 {
            use parity_scale_codec::Encode as _;
            parity_scale_codec::Compact(n).encode()[0]
        }

        assert_eq!(
            obtained,
            vec![
                0x42,          // leaf 0x40 (2^6) with (+) key of 2 nibbles (0x02)
                0xaa,          // key data
                to_compact(1), // length of value in bytes as Compact
                0xbb           // value data
            ]
        );
    }

    #[test]
    fn trie_root_unhashed() {
        let mut trie = Trie::new();
        trie.insert(&[0x48, 0x19], [0xfe].to_vec());
        trie.insert(&[0x13, 0x14], [0xff].to_vec());

        let obtained = trie.node_value(
            TrieNodeKey {
                nibbles: Vec::new(),
            },
            None,
            TrieNodeKey {
                nibbles: Vec::new(),
            },
        );

        fn to_compact(n: u8) -> u8 {
            use parity_scale_codec::Encode as _;
            parity_scale_codec::Compact(n).encode()[0]
        }

        let mut ex = Vec::<u8>::new();
        ex.push(0x80); // branch, no value (0b_10..) no nibble
        ex.push(0x12); // slots 1 & 4 are taken from 0-7
        ex.push(0x00); // no slots from 8-15
        ex.push(to_compact(0x05)); // first slot: LEAF, 5 bytes long.
        ex.push(0x43); // leaf 0x40 with 3 nibbles
        ex.push(0x03); // first nibble
        ex.push(0x14); // second & third nibble
        ex.push(to_compact(0x01)); // 1 byte data
        ex.push(0xff); // value data
        ex.push(to_compact(0x05)); // second slot: LEAF, 5 bytes long.
        ex.push(0x43); // leaf with 3 nibbles
        ex.push(0x08); // first nibble
        ex.push(0x19); // second & third nibble
        ex.push(to_compact(0x01)); // 1 byte data
        ex.push(0xfe); // value data

        assert_eq!(obtained, ex);
    }
}