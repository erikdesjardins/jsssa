use crate::collections::InvertibleSet;

mod stacked_map {
    use crate::collections::StackedMap;

    #[test]
    fn basic() {
        let mut m = StackedMap::default();
        m.insert_self(0, 1);
        assert_eq!(m.get_self(&0), Some(&1));
        assert_eq!(m.get_all(&0), Some(&1));
        assert_eq!(m.get_self(&2), None);
        assert_eq!(m.get_all(&2), None);
        m.remove_self(&0);
        assert_eq!(m.get_self(&0), None);
        assert_eq!(m.get_all(&0), None);
    }

    #[test]
    fn child() {
        let mut m = StackedMap::default();
        m.insert_self(0, 0);
        let mut m1 = m.child();
        m1.insert_self(1, 1);
        let mut m2 = m1.child();
        m2.insert_self(2, 2);
        assert_eq!(m2.get_self(&0), None);
        assert_eq!(m2.get_self(&1), None);
        assert_eq!(m2.get_self(&2), Some(&2));
        assert_eq!(m2.get_all(&0), Some(&0));
        assert_eq!(m2.get_all(&1), Some(&1));
        assert_eq!(m2.get_self(&2), Some(&2));
        m2.clear_self();
        assert_eq!(m2.get_all(&0), Some(&0));
        assert_eq!(m2.get_all(&1), Some(&1));
        assert_eq!(m2.get_self(&2), None);
        m1.remove_self(&0);
        assert_eq!(m1.get_all(&0), Some(&0));
        assert_eq!(m1.get_all(&1), Some(&1));
        m1.remove_self(&1);
        assert_eq!(m1.get_all(&0), Some(&0));
        assert_eq!(m1.get_all(&1), None);
    }

    #[test]
    fn child_limited_lifetime() {
        let m = StackedMap::default();
        let mut m1 = m.child();
        {
            let mut m2 = m1.child();
            m2.insert_self(2, 2);
        }
        m1.insert_self(1, 1);
    }
}

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
