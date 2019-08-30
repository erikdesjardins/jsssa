use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::iter::{FromIterator, FusedIterator};
use std::mem;
use std::option;
use std::vec;

/// Container holding 0/1/n items, avoiding allocation in the 0/1 cases
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
    pub fn child(&self) -> StackedMap<'_, K, V> {
        StackedMap {
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

/// Set capable of representing normal "X ∈ {A, B, C}" but also "X ∉ {X, Y, Z}"
pub struct InvertibleSet<T>(InvertibleSetInner<T>)
where
    T: Eq + Hash;

enum InvertibleSetInner<T>
where
    T: Eq + Hash,
{
    Included(HashSet<T>),
    Excluded(HashSet<T>),
}

impl<T> Default for InvertibleSet<T>
where
    T: Eq + Hash,
{
    fn default() -> Self {
        Self(InvertibleSetInner::Included(Default::default()))
    }
}

impl<T> InvertibleSet<T>
where
    T: Eq + Hash,
{
    pub fn from_included(values: impl IntoIterator<Item = T>) -> Self {
        Self(InvertibleSetInner::Included(values.into_iter().collect()))
    }

    pub fn from_excluded(values: impl IntoIterator<Item = T>) -> Self {
        Self(InvertibleSetInner::Excluded(values.into_iter().collect()))
    }

    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq,
    {
        match &self.0 {
            InvertibleSetInner::Included(included) => included.contains(value),
            InvertibleSetInner::Excluded(excluded) => !excluded.contains(value),
        }
    }

    pub fn insert(&mut self, value: T) -> bool {
        match &mut self.0 {
            InvertibleSetInner::Included(included) => included.insert(value),
            InvertibleSetInner::Excluded(excluded) => excluded.remove(&value),
        }
    }

    pub fn remove(&mut self, value: T) -> bool {
        match &mut self.0 {
            InvertibleSetInner::Included(included) => included.remove(&value),
            InvertibleSetInner::Excluded(excluded) => excluded.insert(value),
        }
    }

    pub fn insert_everything_except(&mut self, values: impl IntoIterator<Item = T>) {
        self.union_with(Self::from_excluded(values));
    }

    pub fn union_with(&mut self, other: Self) {
        match (&mut self.0, other.0) {
            // both included: union
            (InvertibleSetInner::Included(this), InvertibleSetInner::Included(ref mut other)) => {
                this.extend(other.drain());
            }
            // both excluded: intersection
            (InvertibleSetInner::Excluded(this), InvertibleSetInner::Excluded(ref mut other)) => {
                let (smaller, larger) = if this.len() < other.len() {
                    (this, other)
                } else {
                    (other, this)
                };
                let intersection = smaller.drain().filter(|v| larger.contains(v)).collect();
                self.0 = InvertibleSetInner::Excluded(intersection);
            }
            // one of each: remove included from excluded
            (
                InvertibleSetInner::Included(ref this_incl),
                InvertibleSetInner::Excluded(ref mut other_excl),
            ) => {
                // *sigh*, "cannot bind by-move and by-ref in the same pattern"
                let mut other_excl = mem::replace(other_excl, Default::default());
                for v in this_incl {
                    other_excl.remove(v);
                }
                self.0 = InvertibleSetInner::Excluded(other_excl);
            }
            (
                InvertibleSetInner::Excluded(this_excl),
                InvertibleSetInner::Included(ref other_incl),
            ) => {
                for v in other_incl {
                    this_excl.remove(v);
                }
            }
        };
    }
}
