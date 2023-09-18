use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::sync::RwLock;

pub(crate) struct OrderedMap<K, V> {
    inner: RwLock<BTreeMap<K, V>>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            inner: RwLock::default(),
        }
    }
}

impl<K, V> OrderedMap<K, V> {
    pub(crate) fn first_key(&self) -> Option<K>
    where
        K: Ord + Copy,
    {
        let guard = self.inner.read().unwrap();
        guard.first_key_value().map(|pair| pair.0).copied()
    }

    pub(crate) fn pop_first(&self) -> Option<(K, V)>
    where
        K: Ord,
    {
        let mut guard = self.inner.write().unwrap();
        guard.pop_first()
    }

    pub(crate) fn insert(&self, key: K, value: V)
    where
        K: Ord,
    {
        let mut guard = self.inner.write().unwrap();
        guard.insert(key, value);
    }

    pub(crate) fn update(&self, key: K, update_value_fn: impl FnOnce(V) -> V)
    where
        K: Ord,
    {
        let mut guard = self.inner.write().unwrap();
        if let Some(new) = guard.remove(&key).map(update_value_fn) {
            guard.insert(key, new);
        }
    }

    pub(crate) fn delete<Q>(&self, key: &Q)
    where
        K: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        let mut guard = self.inner.write().unwrap();
        guard.remove(key);
    }
}
