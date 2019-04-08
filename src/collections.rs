use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::iter::{FromIterator, FusedIterator};
use std::option;
use std::vec;

/// Container holding 0/1/n items, avoiding allocation in the 0/1 cases
#[allow(dead_code)]
pub enum ZeroOneMany<T> {
    Zero,
    One(T),
    Many(Vec<T>),
}

impl<T> IntoIterator for ZeroOneMany<T> {
    type Item = T;
    type IntoIter = ZeroOneManyIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ZeroOneMany::Zero => ZeroOneManyIter::Option(None.into_iter()),
            ZeroOneMany::One(x) => ZeroOneManyIter::Option(Some(x).into_iter()),
            ZeroOneMany::Many(v) => ZeroOneManyIter::Vec(v.into_iter()),
        }
    }
}

pub enum ZeroOneManyIter<T> {
    Option(option::IntoIter<T>),
    Vec(vec::IntoIter<T>),
}

impl<T> Iterator for ZeroOneManyIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            ZeroOneManyIter::Option(o) => o.next(),
            ZeroOneManyIter::Vec(v) => v.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            ZeroOneManyIter::Option(o) => o.size_hint(),
            ZeroOneManyIter::Vec(v) => v.size_hint(),
        }
    }
}

impl<T> DoubleEndedIterator for ZeroOneManyIter<T> {
    fn next_back(&mut self) -> Option<T> {
        match self {
            ZeroOneManyIter::Option(o) => o.next_back(),
            ZeroOneManyIter::Vec(v) => v.next_back(),
        }
    }
}

impl<T> ExactSizeIterator for ZeroOneManyIter<T> {}

impl<T> FusedIterator for ZeroOneManyIter<T> {}

/// The fundamental sum type
pub enum Either<A, B> {
    A(A),
    B(B),
}

/// A HashMap holding a reference to its parent, allowing efficient scope-based lookup.
pub struct StackedMap<'a, K, V>
where
    K: Eq + Hash,
{
    parent: Option<&'a Self>,
    map: HashMap<K, V>,
}

impl<'a, K, V> Default for StackedMap<'a, K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self {
            parent: None,
            map: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl<'a, K, V> StackedMap<'a, K, V>
where
    K: Eq + Hash,
{
    pub fn child(&'a self) -> Self {
        Self {
            parent: Some(self),
            map: Default::default(),
        }
    }

    pub fn get_all<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get_self(k)
            .or_else(|| self.parent.and_then(|p| p.get_all(k)))
    }

    pub fn get_self<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.get(k)
    }

    pub fn insert_self(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    pub fn remove_self<Q>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.remove(k)
    }

    pub fn clear_self(&mut self) {
        self.map.clear();
    }
}

impl<'a, K, V> FromIterator<(K, V)> for StackedMap<'a, K, V>
where
    K: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self {
            parent: None,
            map: HashMap::from_iter(iter),
        }
    }
}
