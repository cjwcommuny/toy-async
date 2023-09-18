use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct Id<T> {
    id: u64,
    phantom: PhantomData<T>,
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> Eq for Id<T> {}

impl<T> PartialEq<Self> for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl<T> PartialOrd<Self> for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

#[derive(Default)]
pub(crate) struct IdGenerator {
    next: AtomicU64,
}

impl IdGenerator {
    pub(crate) fn next<T>(&self) -> Id<T> {
        Id {
            id: self.next.fetch_add(1, Ordering::SeqCst),
            phantom: PhantomData,
        }
    }
}
