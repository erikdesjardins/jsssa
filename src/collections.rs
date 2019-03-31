use std::iter::FusedIterator;
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
