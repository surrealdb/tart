use std::sync::Arc;

use crate::{Prefix, VecArray};

/*
    Immutable nodes
*/

pub trait NodeTrait<N> {
    fn clone(&self) -> Self;
    fn add_child(&self, key: u8, node: N) -> Self;
    fn find_child(&self, key: u8) -> Option<&Arc<N>>;
    fn delete_child(&self, key: u8) -> Self;
    fn num_children(&self) -> usize;
    fn size(&self) -> usize;
    fn replace_child(&self, key: u8, node: Arc<N>) -> Self;
}

pub trait Timestamp {
    fn ts(&self) -> u64;
}

#[derive(Clone)]
pub struct TwigNode<K: Prefix + Clone, V: Clone> {
    pub prefix: K,
    pub values: Vec<Arc<LeafValue<K, V>>>,
    pub ts: u64, // Timestamp for the twig node
}

#[derive(Clone)]
pub struct LeafValue<K: Prefix + Clone,V: Clone> {
    pub(crate) key: K,
    pub(crate) value: V,
    pub(crate) ts: u64,
}

impl<K: Prefix + Clone, V: Clone> LeafValue<K, V> {
    pub fn new(key:K, value: V, ts: u64) -> Self {
        LeafValue { key, value, ts }
    }
}


impl<K: Prefix + Clone, V: Clone> TwigNode<K, V> {
    pub fn new(prefix: K) -> Self {
        let new_node = TwigNode {
            prefix: prefix,
            values: Vec::new(),
            ts: 0,
        };

        new_node
    }

    pub fn clone(&self) -> Self {
        Self {
            prefix: self.prefix.clone(),
            values: self.values.clone(),
            ts: self.ts,
        }
    }

    pub fn ts(&self) -> u64 {
        self.values.iter().map(|value| value.ts).max().unwrap_or(self.ts)
    }

    // TODO: write tests for this func
    pub fn insert(&self, key: &K, value: V, ts: u64) -> TwigNode<K, V> {
        let mut new_values = self.values.clone();
        
        let new_leaf_value = LeafValue::new(key.clone(), value, ts);

        if let Ok(existing_index) = new_values.binary_search_by(|v| v.key.cmp(&key)) {
            // Update existing key with new value and ts
            new_values[existing_index] = Arc::new(new_leaf_value);
        } else {
            // Insert new LeafValue in sorted order
            let insertion_index = match new_values.binary_search_by(|v| v.ts.cmp(&new_leaf_value.ts)) {
                Ok(index) => index,
                Err(index) => index,
            };
            new_values.insert(insertion_index, Arc::new(new_leaf_value));
        }

        let new_ts = new_values.iter().map(|value| value.ts).max().unwrap_or(self.ts);
        TwigNode {
            prefix: self.prefix.clone(),
            values: new_values,
            ts: new_ts,
        }
    }


    // TODO: write tests for this func
    pub fn insert_mut(&mut self, key: &K, value: V, ts: u64) {
        let new_leaf_value = LeafValue::new(key.clone(), value, ts);

        if let Ok(existing_index) = self.values.binary_search_by(|v| v.key.cmp(&key)) {
            // Update existing key with new value and ts
            self.values[existing_index] = Arc::new(new_leaf_value);
        } else {
            // Insert new LeafValue in sorted order
            let insertion_index = match self.values.binary_search_by(|v| v.ts.cmp(&new_leaf_value.ts)) {
                Ok(index) => index,
                Err(index) => index,
            };
            self.values.insert(insertion_index, Arc::new(new_leaf_value));
        }

        self.ts = self.ts(); // Update LeafNode's timestamp
    }

    // TODO: write tests for this func
    pub fn get_latest_leaf(&self, key: &K) -> Option<Arc<LeafValue<K, V>>> {
        self.values
            .iter()
            .filter(|value| value.key.cmp(key) == std::cmp::Ordering::Equal)
            .max_by_key(|value| value.ts).cloned()
    }

    // TODO: write tests for this func
    pub fn get_latest_value(&self, key: &K) -> Option<V> {
        self.values
            .iter()
            .filter(|value| value.key.cmp(key) == std::cmp::Ordering::Equal)
            .max_by_key(|value| value.ts)
            .map(|value| value.value.clone())
    }

    // TODO: write tests for this func
    pub fn get_value_by_ts(&self, key: &K, timestamp: u64) -> Option<Arc<LeafValue<K, V>>> {
        self.values
            .iter()
            .filter(|value| value.key.cmp(key) == std::cmp::Ordering::Equal && value.ts <= timestamp)
            .max_by_key(|value| value.ts).cloned()
    }
}

impl<K: Prefix + Clone, V: Clone> Timestamp for TwigNode<K, V> {
    fn ts(&self) -> u64 {
        self.ts
    }
}

// Source: https://www.the-paper-trail.org/post/art-paper-notes/
//
// Node4: For nodes with up to four children, ART stores all the keys in a list,
// and the child pointers in a parallel list. Looking up the next character
// in a string means searching the list of child keys, and then using the
// index to look up the corresponding pointer.
//
// Node16: Keys in a Node16 are stored sorted, so binary search could be used to
// find a particular key. Nodes with from 5 to 16 children have an identical layout
// to Node4, just with 16 children per node
//
// A FlatNode is a node with a fixed number of children. It is used for nodes with
// more than 16 children. The children are stored in a fixed-size array, and the
// keys are stored in a parallel array. The keys are stored in sorted order, so
// binary search can be used to find a particular key. The FlatNode is used for
// storing Node4 and Node16 since they have identical layouts.

pub struct FlatNode<P: Prefix + Clone, N: Timestamp, const WIDTH: usize> {
    pub prefix: P,
    pub ts: u64,
    keys: [u8; WIDTH],
    children: Vec<Option<Arc<N>>>,
    num_children: u8,
}

impl<P: Prefix + Clone, N: Timestamp, const WIDTH: usize> FlatNode<P, N, WIDTH> {
    pub fn new(prefix: P) -> Self {
        Self {
            prefix,
            ts: 0,
            keys: [0; WIDTH],
            children: vec![None; WIDTH],
            num_children: 0,
        }
    }

    fn find_pos(&self, key: u8) -> Option<usize> {
        let idx = (0..self.num_children as usize)
            .rev()
            .find(|&i| key < self.keys[i]);
        idx.or(Some(self.num_children as usize))
    }

    fn index(&self, key: u8) -> Option<usize> {
        self.keys[..std::cmp::min(WIDTH, self.num_children as usize)]
            .iter()
            .position(|&c| key == c)
    }

    pub fn resize<const NEW_WIDTH: usize>(&self) -> FlatNode<P, N, NEW_WIDTH> {
        let mut new_node = FlatNode::<P, N, NEW_WIDTH>::new(self.prefix.clone());
        for i in 0..self.num_children as usize {
            new_node.keys[i] = self.keys[i];
            new_node.children[i] = self.children[i].clone();
        }
        new_node.ts = self.ts;
        new_node.num_children = self.num_children;
        new_node.update_ts();
        new_node
    }

    pub fn grow(&self) -> Node48<P, N> {
        let mut n48 = Node48::new(self.prefix.clone());
        for i in 0..self.num_children as usize {
            if let Some(child) = self.children[i].as_ref() {
                n48.insert_child(self.keys[i], child.clone());
            }
        }
        n48.update_ts();
        n48
    }

    // Helper function to insert a child node at the specified position
    #[inline]
    fn insert_child(&mut self, idx: usize, key: u8, node: Arc<N>) {
        for i in (idx..self.num_children as usize).rev() {
            self.keys[i + 1] = self.keys[i];
            self.children[i + 1] = self.children[i].clone();
        }
        self.keys[idx] = key;
        self.children[idx] = Some(node);
        self.num_children += 1;
    }

    #[inline]
    fn max_child_ts(&self) -> u64 {
        self.children.iter().fold(0, |acc, x| {
            if let Some(child) = x.as_ref() {
                std::cmp::max(acc, child.ts())
            } else {
                acc
            }
        })
    }

    #[inline]
    fn update_ts_to_max_child_ts(&mut self) {
        self.ts = self.max_child_ts();
    }

    #[inline]
    fn update_ts(&mut self) {
        // Compute the maximum timestamp among all children
        let max_child_ts = self.max_child_ts();

        // If self.ts is less than the maximum child timestamp, update it.
        if self.ts < max_child_ts {
            self.ts = max_child_ts;
        }
    }

    #[inline]
    fn update_if_newer(&mut self, new_ts: u64) {
        if new_ts > self.ts {
            self.ts = new_ts;
        }
    }

    #[inline]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u8, &Arc<N>)> {
        self.keys
            .iter()
            .zip(self.children.iter())
            .take(self.num_children as usize)
            .map(|(&k, c)| (k, c.as_ref().unwrap()))
    }
}

impl<P: Prefix + Clone, N: Timestamp, const WIDTH: usize> NodeTrait<N> for FlatNode<P, N, WIDTH> {
    fn clone(&self) -> Self {
        let mut new_node = Self::new(self.prefix.clone());
        for i in 0..self.num_children as usize {
            new_node.keys[i] = self.keys[i];
            new_node.children[i] = self.children[i].clone();
        }
        new_node.num_children = self.num_children;
        new_node.ts = self.ts;
        new_node
    }

    fn replace_child(&self, key: u8, node: Arc<N>) -> Self {
        let mut new_node = self.clone();
        let idx = new_node.index(key).unwrap();
        new_node.keys[idx] = key;
        new_node.children[idx] = Some(node);
        new_node.update_ts_to_max_child_ts();

        new_node
    }

    fn add_child(&self, key: u8, node: N) -> Self {
        let mut new_node = self.clone();
        let idx = self.find_pos(key).expect("node is full");

        // Update the timestamp if the new child has a greater timestamp
        new_node.update_if_newer(node.ts());

        // Convert the node to Arc<N> and insert it
        new_node.insert_child(idx, key, Arc::new(node));
        new_node
    }

    fn find_child(&self, key: u8) -> Option<&Arc<N>> {
        let idx = self.index(key)?;
        let child = self.children[idx].as_ref();
        child
    }

    fn delete_child(&self, key: u8) -> Self {
        let mut new_node = self.clone();
        let idx = self
            .keys
            .iter()
            .take(self.num_children as usize)
            .position(|&k| k == key)
            .unwrap();

        new_node.children[idx] = None;

        for i in idx..(WIDTH - 1) {
            new_node.keys[i] = self.keys[i + 1];
            new_node.children[i] = self.children[i + 1].clone();
        }

        new_node.keys[WIDTH - 1] = 0;
        new_node.children[WIDTH - 1] = None;
        new_node.num_children -= 1;
        new_node.update_ts_to_max_child_ts();

        new_node
    }

    #[inline(always)]
    fn num_children(&self) -> usize {
        self.num_children as usize
    }

    #[inline(always)]
    fn size(&self) -> usize {
        WIDTH
    }
}

impl<P: Prefix + Clone, N: Timestamp, const WIDTH: usize> Timestamp for FlatNode<P, N, WIDTH> {
    fn ts(&self) -> u64 {
        self.ts
    }
}

// Source: https://www.the-paper-trail.org/post/art-paper-notes/
//
// Node48: It can hold up to three times as many keys as a Node16. As the paper says,
// when there are more than 16 children, searching for the key can become expensive,
// so instead the keys are stored implicitly in an array of 256 indexes. The entries
// in that array index a separate array of up to 48 pointers.
//
// A Node48 is a 256-entry array of pointers to children. The pointers are stored in
// a Vector Array, which is a Vector of length WIDTH (48) that stores the pointers.

pub struct Node48<P: Prefix + Clone, N: Timestamp> {
    pub prefix: P,
    pub ts: u64,
    child_ptr_indexes: Box<VecArray<u8, 256>>,
    children: Box<VecArray<Arc<N>, 48>>,
    num_children: u8,
}

impl<P: Prefix + Clone, N: Timestamp> Node48<P, N> {
    pub fn new(prefix: P) -> Self {
        Self {
            prefix,
            ts: 0,
            child_ptr_indexes: Box::new(VecArray::new()),
            children: Box::new(VecArray::new()),
            num_children: 0,
        }
    }

    pub fn insert_child(&mut self, key: u8, node: Arc<N>) {
        let pos = self.children.first_free_pos();

        self.child_ptr_indexes.set(key as usize, pos as u8);
        self.children.set(pos, node);
        self.num_children += 1;
    }

    pub fn shrink<const NEW_WIDTH: usize>(&self) -> FlatNode<P, N, NEW_WIDTH> {
        let mut fnode = FlatNode::new(self.prefix.clone());
        for (key, pos) in self.child_ptr_indexes.iter() {
            let child = self.children.get(*pos as usize).unwrap().clone();
            let idx = fnode.find_pos(key as u8).expect("node is full");
            fnode.insert_child(idx, key as u8, child);
        }
        fnode.update_ts();
        fnode
    }

    pub fn grow(&self) -> Node256<P, N> {
        let mut n256 = Node256::new(self.prefix.clone());
        for (key, pos) in self.child_ptr_indexes.iter() {
            let child = self.children.get(*pos as usize).unwrap().clone();
            n256.insert_child(key as u8, child);
        }
        n256.update_ts();
        n256
    }

    #[inline]
    fn max_child_ts(&self) -> u64 {
        self.children
            .iter()
            .fold(0, |acc, x| std::cmp::max(acc, x.1.ts()))
    }

    #[inline]
    fn update_ts_to_max_child_ts(&mut self) {
        self.ts = self.max_child_ts();
    }

    #[inline]
    fn update_ts(&mut self) {
        // Compute the maximum timestamp among all children
        let max_child_ts = self.max_child_ts();

        // If self.ts is less than the maximum child timestamp, update it.
        if self.ts < max_child_ts {
            self.ts = max_child_ts;
        }
    }

    #[inline]
    fn update_if_newer(&mut self, new_ts: u64) {
        if new_ts > self.ts {
            self.ts = new_ts;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (u8, &Arc<N>)> {
        self.child_ptr_indexes
            .iter()
            .map(move |(key, pos)| (key as u8, self.children.get(*pos as usize).unwrap()))
    }
}

impl<P: Prefix + Clone, N: Timestamp> NodeTrait<N> for Node48<P, N> {
    fn clone(&self) -> Self {
        Node48 {
            prefix: self.prefix.clone(),
            ts: self.ts,
            child_ptr_indexes: Box::new(*self.child_ptr_indexes.clone()),
            children: Box::new(*self.children.clone()),
            num_children: self.num_children,
        }
    }

    fn replace_child(&self, key: u8, node: Arc<N>) -> Self {
        let mut new_node = self.clone();
        let idx = new_node.child_ptr_indexes.get(key as usize).unwrap();
        new_node.children.set(*idx as usize, node);
        new_node.update_ts_to_max_child_ts();

        new_node
    }

    fn add_child(&self, key: u8, node: N) -> Self {
        let mut new_node = self.clone();

        // Update the timestamp if the new child has a greater timestamp
        new_node.update_if_newer(node.ts());

        new_node.insert_child(key, Arc::new(node));
        new_node
    }

    fn delete_child(&self, key: u8) -> Self {
        let pos = self.child_ptr_indexes.get(key as usize).unwrap();
        let mut new_node = self.clone();
        new_node.child_ptr_indexes.erase(key as usize);
        new_node.children.erase(*pos as usize);
        new_node.num_children -= 1;

        new_node.update_ts_to_max_child_ts();
        new_node
    }

    fn find_child(&self, key: u8) -> Option<&Arc<N>> {
        let idx = self.child_ptr_indexes.get(key as usize)?;
        let child = self.children.get(*idx as usize)?;
        Some(child)
    }

    fn num_children(&self) -> usize {
        self.num_children as usize
    }

    #[inline]
    fn size(&self) -> usize {
        48
    }
}

impl<P: Prefix + Clone, N: Timestamp> Timestamp for Node48<P, N> {
    fn ts(&self) -> u64 {
        self.ts
    }
}

// Source: https://www.the-paper-trail.org/post/art-paper-notes/
//
// Node256: It is the traditional trie node, used when a node has
// between 49 and 256 children. Looking up child pointers is obviously
// very efficient - the most efficient of all the node types - and when
// occupancy is at least 49 children the wasted space is less significant.
//
// A Node256 is a 256-entry array of pointers to children. The pointers are stored in
// a Vector Array, which is a Vector of length WIDTH (256) that stores the pointers.
pub struct Node256<P: Prefix + Clone, N: Timestamp> {
    pub prefix: P, // Prefix associated with the node
    pub ts: u64,   // Timestamp for node256

    children: Box<VecArray<Arc<N>, 256>>,
    num_children: usize,
}

impl<P: Prefix + Clone, N: Timestamp> Node256<P, N> {
    pub fn new(prefix: P) -> Self {
        Self {
            prefix,
            ts: 0,
            children: Box::new(VecArray::new()),
            num_children: 0,
        }
    }

    pub fn shrink(&self) -> Node48<P, N> {
        let mut indexed = Node48::new(self.prefix.clone());
        let keys: Vec<usize> = self.children.iter_keys().collect();
        for key in keys {
            let child = self.children.get(key).unwrap().clone();
            indexed.insert_child(key as u8, child);
        }
        indexed.update_ts();
        indexed
    }

    #[inline]
    fn insert_child(&mut self, key: u8, node: Arc<N>) {
        self.children.set(key as usize, node);
        self.num_children += 1;
    }

    #[inline]
    fn max_child_ts(&self) -> u64 {
        self.children
            .iter()
            .fold(0, |acc, x| std::cmp::max(acc, x.1.ts()))
    }

    #[inline]
    fn update_ts_to_max_child_ts(&mut self) {
        self.ts = self.max_child_ts();
    }

    #[inline]
    fn update_ts(&mut self) {
        // Compute the maximum timestamp among all children
        let max_child_ts = self.max_child_ts();

        // If self.ts is less than the maximum child timestamp, update it.
        if self.ts < max_child_ts {
            self.ts = max_child_ts;
        }
    }

    #[inline]
    fn update_if_newer(&mut self, new_ts: u64) {
        if new_ts > self.ts {
            self.ts = new_ts;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (u8, &Arc<N>)> {
        self.children.iter().map(|(key, node)| (key as u8, node))
    }
}

impl<P: Prefix + Clone, N: Timestamp> NodeTrait<N> for Node256<P, N> {
    fn clone(&self) -> Self {
        Self {
            prefix: self.prefix.clone(),
            ts: self.ts,
            children: self.children.clone(),
            num_children: self.num_children,
        }
    }

    fn replace_child(&self, key: u8, node: Arc<N>) -> Self {
        let mut new_node = self.clone();

        new_node.children.set(key as usize, node);
        new_node.update_ts_to_max_child_ts();
        new_node
    }

    #[inline]
    fn add_child(&self, key: u8, node: N) -> Self {
        let mut new_node = self.clone();

        // Update the timestamp if the new child has a greater timestamp
        new_node.update_if_newer(node.ts());

        new_node.insert_child(key, Arc::new(node));
        new_node
    }

    #[inline]
    fn find_child(&self, key: u8) -> Option<&Arc<N>> {
        let child = self.children.get(key as usize)?;
        Some(child)
    }

    #[inline]
    fn delete_child(&self, key: u8) -> Self {
        let mut new_node = self.clone();
        let removed = new_node.children.erase(key as usize);
        if removed.is_some() {
            new_node.num_children -= 1;
        }
        new_node.update_ts_to_max_child_ts();
        new_node
    }

    #[inline]
    fn num_children(&self) -> usize {
        self.num_children
    }

    fn size(&self) -> usize {
        256
    }
}

impl<P: Prefix + Clone, N: Timestamp> Timestamp for Node256<P, N> {
    fn ts(&self) -> u64 {
        self.ts
    }
}

#[cfg(test)]
mod tests {
    use super::{FlatNode, TwigNode, Node256, Node48, NodeTrait, Timestamp, VecArray};
    use crate::ArrayPrefix;
    use std::sync::Arc;

    macro_rules! impl_timestamp {
        ($($t:ty),*) => {
            $(
                impl Timestamp for $t {
                    fn ts(&self) -> u64 {
                        *self as u64
                    }
                }
            )*
        };
    }

    impl_timestamp!(usize, u8, u16, u32, u64);

    #[test]
    fn new() {
        let v: VecArray<i32, 10> = VecArray::new();
        assert_eq!(v.storage.capacity(), 10);
    }

    #[test]
    fn push_and_pop() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        let index = v.push(5);
        assert_eq!(v.get(index), Some(&5));
        assert_eq!(v.pop(), Some(5));
    }

    #[test]
    fn last() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        v.push(6);
        assert_eq!(v.last(), Some(&6));
    }

    #[test]
    fn last_used_pos() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        v.push(6);
        assert_eq!(v.last_used_pos(), Some(1));
    }

    #[test]
    fn first_free_pos() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        assert_eq!(v.first_free_pos(), 1);
    }

    #[test]
    fn get_and_set() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.set(5, 6);
        assert_eq!(v.get(5), Some(&6));
    }

    #[test]
    fn get_mut() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.set(5, 6);
        if let Some(value) = v.get_mut(5) {
            *value = 7;
        }
        assert_eq!(v.get(5), Some(&7));
    }

    #[test]
    fn erase() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        assert_eq!(v.erase(0), Some(5));
        assert_eq!(v.get(0), None);
    }

    #[test]
    fn clear() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        v.clear();
        assert!(v.is_empty());
    }

    #[test]
    fn is_empty() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        assert!(v.is_empty());
        v.push(5);
        assert!(!v.is_empty());
    }

    #[test]
    fn iter_keys() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        v.push(6);
        let keys: Vec<usize> = v.iter_keys().collect();
        assert_eq!(keys, vec![0, 1]);
    }

    #[test]
    fn iter() {
        let mut v: VecArray<i32, 10> = VecArray::new();
        v.push(5);
        v.push(6);
        let values: Vec<(usize, &i32)> = v.iter().collect();
        assert_eq!(values, vec![(0, &5), (1, &6)]);
    }

    fn node_test(mut node: impl NodeTrait<usize>, size: usize) {
        for i in 0..size {
            node = node.add_child(i as u8, i);
        }

        for i in 0..size {
            assert!(matches!(node.find_child(i as u8), Some(v) if *v == i.into()));
        }

        for i in 0..size {
            node = node.delete_child(i as u8);
        }

        assert!(matches!(node.num_children(), 0));
    }

    #[test]
    fn test_flatnode() {
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        node_test(
            FlatNode::<ArrayPrefix<8>, usize, 4>::new(dummy_prefix.clone()),
            4,
        );
        node_test(
            FlatNode::<ArrayPrefix<8>, usize, 16>::new(dummy_prefix.clone()),
            16,
        );
        node_test(
            FlatNode::<ArrayPrefix<8>, usize, 32>::new(dummy_prefix.clone()),
            32,
        );
        node_test(
            FlatNode::<ArrayPrefix<8>, usize, 48>::new(dummy_prefix.clone()),
            48,
        );
        node_test(
            FlatNode::<ArrayPrefix<8>, usize, 64>::new(dummy_prefix.clone()),
            64,
        );

        // resize from 16 to 4
        let mut node = FlatNode::<ArrayPrefix<8>, usize, 16>::new(dummy_prefix.clone());
        for i in 0..4 {
            node = node.add_child(i as u8, i);
        }

        let resized: FlatNode<ArrayPrefix<8>, usize, 4> = node.resize();
        assert_eq!(resized.num_children, 4);
        for i in 0..4 {
            assert!(matches!(resized.find_child(i as u8), Some(v) if *v == i.into()));
        }

        // resize from 4 to 16
        let mut node = FlatNode::<ArrayPrefix<8>, usize, 4>::new(dummy_prefix.clone());
        for i in 0..4 {
            node = node.add_child(i as u8, i);
        }
        let mut resized: FlatNode<ArrayPrefix<8>, usize, 16> = node.resize();
        assert_eq!(resized.num_children, 4);
        for i in 4..16 {
            resized = resized.add_child(i as u8, i);
        }
        assert_eq!(resized.num_children, 16);
        for i in 0..16 {
            assert!(matches!(resized.find_child(i as u8), Some(v) if *v == i.into()));
        }

        // resize from 16 to 48
        let mut node = FlatNode::<ArrayPrefix<8>, usize, 16>::new(dummy_prefix.clone());
        for i in 0..16 {
            node = node.add_child(i as u8, i);
        }

        let resized = node.grow();
        assert_eq!(resized.num_children, 16);
        for i in 0..16 {
            assert!(matches!(resized.find_child(i as u8), Some(v) if *v == i.into()));
        }

        let mut node = FlatNode::<ArrayPrefix<8>, usize, 4>::new(dummy_prefix);
        node = node.add_child(1, 1);
        node = node.add_child(2, 2);
        node = node.add_child(3, 3);
        node = node.add_child(4, 4);
        assert_eq!(node.num_children(), 4);
        assert_eq!(node.find_child(1), Some(&1.into()));
        assert_eq!(node.find_child(2), Some(&2.into()));
        assert_eq!(node.find_child(3), Some(&3.into()));
        assert_eq!(node.find_child(4), Some(&4.into()));
        assert_eq!(node.find_child(5), None);

        node = node.delete_child(1);
        node = node.delete_child(2);
        node = node.delete_child(3);
        node = node.delete_child(4);
        // // assert_eq!(node.delete_child(1), Some(1));
        // // assert_eq!(node.delete_child(2), Some(2));
        // // assert_eq!(node.delete_child(3), Some(3));
        // // assert_eq!(node.delete_child(4), Some(4));
        // // assert_eq!(node.delete_child(5), None);
        assert_eq!(node.num_children(), 0);
    }

    #[test]
    fn test_node48() {
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        // node_test(super::Node48::<usize, 48>::new(), 48);
        let mut n48 = Node48::<ArrayPrefix<8>, u8>::new(dummy_prefix.clone());
        for i in 0..48 {
            n48 = n48.add_child(i, i);
        }
        for i in 0..48 {
            assert_eq!(n48.find_child(i), Some(&i.into()));
        }
        for i in 0..48 {
            n48 = n48.delete_child(i);
        }
        for i in 0..48 {
            assert!(n48.find_child(i as u8).is_none());
        }

        // resize from 48 to 16
        let mut node = Node48::<ArrayPrefix<8>, u8>::new(dummy_prefix.clone());
        for i in 0..18 {
            node = node.add_child(i, i);
        }
        assert_eq!(node.num_children, 18);
        node = node.delete_child(0);
        node = node.delete_child(1);
        assert_eq!(node.num_children, 16);

        let resized = node.shrink::<16>();
        assert_eq!(resized.num_children, 16);
        for i in 2..18 {
            assert!(matches!(resized.find_child(i), Some(v) if *v == i.into()));
        }

        // resize from 48 to 4
        let mut node = Node48::<ArrayPrefix<8>, u8>::new(dummy_prefix.clone());
        for i in 0..4 {
            node = node.add_child(i, i);
        }
        let resized = node.shrink::<4>();
        assert_eq!(resized.num_children, 4);
        for i in 0..4 {
            assert!(matches!(resized.find_child(i), Some(v) if *v == i.into()));
        }

        // resize from 48 to 256
        let mut node = Node48::<ArrayPrefix<8>, u8>::new(dummy_prefix);
        for i in 0..48 {
            node = node.add_child(i, i);
        }

        let resized = node.grow();
        assert_eq!(resized.num_children, 48);
        for i in 0..48 {
            assert!(matches!(resized.find_child(i), Some(v) if *v == i.into()));
        }
    }

    #[test]
    fn test_node256() {
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        node_test(
            Node256::<ArrayPrefix<8>, usize>::new(dummy_prefix.clone()),
            255,
        );

        let mut n256 = Node256::new(dummy_prefix.clone());
        for i in 0..255 {
            n256 = n256.add_child(i, i);
            assert_eq!(n256.find_child(i), Some(&i.into()));
            n256 = n256.delete_child(i);
            assert_eq!(n256.find_child(i), None);
        }

        // resize from 256 to 48
        let mut node = Node256::new(dummy_prefix);
        for i in 0..48 {
            node = node.add_child(i, i);
        }

        let resized = node.shrink();
        assert_eq!(resized.num_children, 48);
        for i in 0..48 {
            assert!(matches!(resized.find_child(i), Some(v) if *v == i.into()));
        }
    }

    #[test]
    fn test_flatnode_update_ts() {
        const WIDTH: usize = 4;
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        // Prepare some child nodes
        let mut child1 = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
        child1.ts = 5;
        let mut child2 = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
        child2.ts = 10;
        let mut child3 = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
        child3.ts = 3;
        let mut child4 = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
        child4.ts = 7;

        let mut parent = FlatNode {
            prefix: dummy_prefix.clone(),
            ts: 6,
            keys: [0; WIDTH],
            children: vec![
                Some(Arc::new(child1)),
                Some(Arc::new(child2)),
                Some(Arc::new(child3)),
                None,
            ],
            num_children: 3,
        };

        // The maximum timestamp among children is 10 (child2.ts), so after calling update_ts,
        // the parent's timestamp should be updated to 10.
        parent.update_ts();
        assert_eq!(parent.ts(), 10);

        // Add a new child with a larger timestamp (15), parent's timestamp should update to 15
        let mut child5 = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
        child5.ts = 15;
        parent = parent.add_child(3, child5);
        assert_eq!(parent.ts(), 15);

        // Delete the child with the largest timestamp, parent's timestamp should update to next max (10)
        parent = parent.delete_child(3);
        assert_eq!(parent.ts(), 10);

        // Update a child's timestamp to be the largest (20), parent's timestamp should update to 20
        let mut child6 = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix);
        child6.ts = 20;
        parent.children[2] = Some(Arc::new(child6));
        parent.update_ts();
        assert_eq!(parent.ts(), 20);
    }

    #[test]
    fn test_flatnode_repeated_update_ts() {
        const WIDTH: usize = 1;
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        let child = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
        let mut parent: FlatNode<ArrayPrefix<8>, FlatNode<ArrayPrefix<8>, usize, 1>, 1> =
            FlatNode {
                prefix: dummy_prefix,
                ts: 6,
                keys: [0; WIDTH],
                children: vec![Some(Arc::new(child))],
                num_children: 1,
            };

        // Calling update_ts once should update the timestamp.
        parent.update_ts();
        let ts_after_first_update = parent.ts();

        // Calling update_ts again should not change the timestamp.
        parent.update_ts();
        assert_eq!(parent.ts(), ts_after_first_update);
    }

    #[test]
    fn test_node48_update_ts() {
        const WIDTH: usize = 4;
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        // Prepare some child nodes with varying timestamps
        let children: Vec<_> = (0..WIDTH)
            .map(|i| {
                let mut child = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
                child.ts = i as u64;
                child
            })
            .collect();

        let mut parent: Node48<ArrayPrefix<8>, FlatNode<ArrayPrefix<8>, usize, WIDTH>> =
            Node48::<ArrayPrefix<8>, FlatNode<ArrayPrefix<8>, usize, WIDTH>>::new(
                dummy_prefix,
            );

        // Add children to parent
        for (i, child) in children.iter().enumerate() {
            parent = parent.add_child(i as u8, child.clone());
        }
        // The maximum timestamp among children is (WIDTH - 1), so after calling update_ts,
        // the parent's timestamp should be updated to (WIDTH - 1).
        parent.update_ts();
        assert_eq!(parent.ts(), (WIDTH - 1) as u64);
    }

    #[test]
    fn test_node256_update_ts() {
        const WIDTH: usize = 256;
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        // Prepare some child nodes with varying timestamps
        let children: Vec<_> = (0..WIDTH)
            .map(|i| {
                let mut child = FlatNode::<ArrayPrefix<8>, usize, WIDTH>::new(dummy_prefix.clone());
                child.ts = i as u64;
                child
            })
            .collect();

        let mut parent: Node256<ArrayPrefix<8>, FlatNode<ArrayPrefix<8>, usize, WIDTH>> =
            Node256::<ArrayPrefix<8>, FlatNode<ArrayPrefix<8>, usize, WIDTH>>::new(
                dummy_prefix,
            );

        // Add children to parent
        for (i, child) in children.iter().enumerate() {
            parent = parent.add_child(i as u8, child.clone());
        }

        // The maximum timestamp among children is (WIDTH - 1), so after calling update_ts,
        // the parent's timestamp should be updated to (WIDTH - 1).
        parent.update_ts();
        assert_eq!(parent.ts(), (WIDTH - 1) as u64);
    }

    // TODO: add more scenarios to this as twig nodes have the actual data with timestamps
    #[test]
    fn test_twig_nodes() {
        const WIDTH: usize = 4;
        let dummy_prefix: ArrayPrefix<8> = ArrayPrefix::create_key("foo".as_bytes());

        // Prepare some child nodes
        let mut twig1 =
            TwigNode::<ArrayPrefix<8>, usize>::new(dummy_prefix.clone());
        twig1.ts = 5;
        let mut twig2 =
            TwigNode::<ArrayPrefix<8>, usize>::new(dummy_prefix.clone());
        twig2.ts = 10;
        let mut twig3 =
            TwigNode::<ArrayPrefix<8>, usize>::new(dummy_prefix.clone());
        twig3.ts = 3;
        let mut twig4 =
            TwigNode::<ArrayPrefix<8>, usize>::new(dummy_prefix.clone());
        twig4.ts = 7;

        let mut parent = FlatNode {
            prefix: dummy_prefix,
            ts: 0,
            keys: [0; WIDTH],
            children: vec![
                Some(Arc::new(twig1)),
                Some(Arc::new(twig2)),
                Some(Arc::new(twig3)),
                Some(Arc::new(twig4)),
            ],
            num_children: 3,
        };

        // The maximum timestamp among children is 10 (child2.ts), so after calling update_ts,
        // the parent's timestamp should be updated to 10.
        parent.update_ts();
        assert_eq!(parent.ts(), 10);
    }
}
