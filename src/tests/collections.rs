use crate::collections::InvertibleSet;

#[test]
fn basic() {
    let mut s = InvertibleSet::default();
    s.insert(1);
    assert!(!s.contains(&0));
    assert!(s.contains(&1));
}

#[test]
fn from_included() {
    let mut s = InvertibleSet::from_included(vec![1]);
    assert!(!s.contains(&0));
    assert!(s.contains(&1));
    s.insert(1);
    assert!(s.contains(&1));
    s.remove(1);
    assert!(!s.contains(&1));
}

#[test]
fn from_excluded() {
    let mut s = InvertibleSet::from_excluded(vec![1]);
    assert!(s.contains(&0));
    assert!(!s.contains(&1));
    s.remove(1);
    assert!(!s.contains(&1));
    s.insert(1);
    assert!(s.contains(&1));
}

#[test]
fn insert_all_except1() {
    let mut s = InvertibleSet::default();
    s.insert(0);
    s.insert_everything_except(vec![1]);
    s.remove(2);
    assert!(s.contains(&0));
    assert!(!s.contains(&1));
    assert!(!s.contains(&2));
    assert!(s.contains(&3));
}

#[test]
fn insert_all_except2() {
    let mut s = InvertibleSet::from_excluded(vec![-1]);
    s.insert(0);
    s.insert_everything_except(vec![1]);
    s.remove(2);
    assert!(s.contains(&-1));
    assert!(s.contains(&0));
    assert!(s.contains(&1));
    assert!(!s.contains(&2));
    assert!(s.contains(&3));
}

#[test]
fn union_with1() {
    let mut s = InvertibleSet::from_included(vec![1, 2]);
    s.union_with(InvertibleSet::from_included(vec![2, 3]));
    assert!(s.contains(&1));
    assert!(s.contains(&2));
    assert!(s.contains(&3));
}

#[test]
fn union_with2() {
    let mut s = InvertibleSet::from_included(vec![1, 2]);
    s.union_with(InvertibleSet::from_excluded(vec![2, 3]));
    assert!(s.contains(&1));
    assert!(s.contains(&2));
    assert!(!s.contains(&3));
}

#[test]
fn union_with3() {
    let mut s = InvertibleSet::from_excluded(vec![1, 2]);
    s.union_with(InvertibleSet::from_included(vec![2, 3]));
    assert!(!s.contains(&1));
    assert!(s.contains(&2));
    assert!(s.contains(&3));
}

#[test]
fn union_with4() {
    let mut s = InvertibleSet::from_excluded(vec![1, 2]);
    s.union_with(InvertibleSet::from_excluded(vec![2, 3]));
    assert!(s.contains(&1));
    assert!(!s.contains(&2));
    assert!(s.contains(&3));
}
