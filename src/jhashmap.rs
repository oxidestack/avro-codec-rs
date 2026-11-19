//! `java.util.HashMap` iteration order: keys by bucket (`(h ^ h >>> 16) & (capacity - 1)`), insertion order within
//! a bucket, the table doubling past a 0.75 load and whenever a bucket reaches eight entries while the table is
//! smaller than 64 (`treeifyBin` resizes instead of treeifying).
//!
//! Buckets that do become trees (nine keys in one bucket of a 64+ table) keep insertion order here; Java moves the
//! tree's root to the front.

/// A map from keys (compared with `Eq`) to values, iterating as a `HashMap` with default capacity would.
pub(crate) struct JavaHashMap<K, V> {
    /// Entries in insertion order with their hash.
    entries: Vec<(i32, K, V)>,
    capacity: usize,
}

const TREEIFY_THRESHOLD: usize = 8;
const MIN_TREEIFY_CAPACITY: usize = 64;

fn spread(h: i32) -> usize {
    (h ^ ((h as u32) >> 16) as i32) as u32 as usize
}

impl<K: PartialEq, V> JavaHashMap<K, V> {
    pub(crate) fn new() -> JavaHashMap<K, V> {
        JavaHashMap { entries: Vec::new(), capacity: 16 }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `put(key, value)`: an existing key keeps its place and takes the new value.
    pub(crate) fn put(&mut self, hash: i32, key: K, value: V) {
        if let Some(e) = self.entries.iter_mut().find(|(h, k, _)| *h == hash && *k == key) {
            e.2 = value;
            return;
        }
        let bucket = spread(hash) & (self.capacity - 1);
        let in_bucket = self.entries.iter().filter(|(h, _, _)| spread(*h) & (self.capacity - 1) == bucket).count();
        self.entries.push((hash, key, value));
        // `binCount >= TREEIFY_THRESHOLD - 1` when appending to a chain of eight.
        if in_bucket >= TREEIFY_THRESHOLD && self.capacity < MIN_TREEIFY_CAPACITY {
            self.capacity *= 2;
        }
        if self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
    }

    pub(crate) fn remove(&mut self, hash: i32, key: &K) -> Option<V> {
        let i = self.entries.iter().position(|(h, k, _)| *h == hash && k == key)?;
        Some(self.entries.remove(i).2)
    }

    /// The entries in iteration order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let mask = self.capacity - 1;
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by_key(|i| spread(self.entries[*i].0) & mask);
        order.into_iter().map(move |i| (&self.entries[i].1, &self.entries[i].2))
    }

    pub(crate) fn into_vec(self) -> Vec<(K, V)> {
        let mask = self.capacity - 1;
        let mut entries: Vec<(usize, K, V)> = self.entries.into_iter().map(|(h, k, v)| (spread(h) & mask, k, v)).collect();
        entries.sort_by_key(|e| e.0);
        entries.into_iter().map(|(_, k, v)| (k, v)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_by_bucket_then_insertion() {
        let mut m = JavaHashMap::new();
        for (h, k) in [(17, "a"), (1, "b"), (3, "c")] {
            m.put(h, k, ());
        }
        assert_eq!(m.iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["a", "b", "c"]);
        m.put(1, "b", ());
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn a_crowded_bucket_resizes_a_small_table() {
        let mut m = JavaHashMap::new();
        // Nine keys in bucket 0 of 16: the ninth doubles the table, splitting 0/16.
        for i in 0..9 {
            m.put(i * 16, i, ());
        }
        assert_eq!(m.capacity, 32);
        assert_eq!(m.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [0, 2, 4, 6, 8, 1, 3, 5, 7]);
    }
}
