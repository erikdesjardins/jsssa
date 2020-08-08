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

mod small_map {
    use crate::collections::SmallMap;

    #[test]
    fn insert() {
        let mut m = SmallMap::default();

        assert_eq!(m.get(&0), None);

        assert_eq!(m.insert(0, 1), None);
        assert_eq!(m.get(&0), Some(&1));
        assert_eq!(m.get(&1), None);

        assert_eq!(m.insert(0, 2), Some(1));
        assert_eq!(m.get(&0), Some(&2));
        assert_eq!(m.get(&1), None);

        assert_eq!(m.insert(1, 3), None);
        assert_eq!(m.get(&0), Some(&2));
        assert_eq!(m.get(&1), Some(&3));
        assert_eq!(m.get(&2), None);

        assert_eq!(m.insert(1, 4), Some(3));
        assert_eq!(m.get(&0), Some(&2));
        assert_eq!(m.get(&1), Some(&4));
        assert_eq!(m.get(&2), None);

        assert_eq!(m.insert(2, 5), None);
        assert_eq!(m.get(&0), Some(&2));
        assert_eq!(m.get(&1), Some(&4));
        assert_eq!(m.get(&2), Some(&5));
        assert_eq!(m.get(&3), None);

        assert_eq!(m.insert(2, 6), Some(5));
        assert_eq!(m.get(&0), Some(&2));
        assert_eq!(m.get(&1), Some(&4));
        assert_eq!(m.get(&2), Some(&6));
        assert_eq!(m.get(&3), None);
    }

    #[test]
    fn remove() {
        let mut m = SmallMap::default();

        assert_eq!(m.remove(&0), None);

        assert_eq!(m.insert(0, 1), None);
        assert_eq!(m.remove(&1), None);
        assert_eq!(m.remove(&0), Some(1));

        assert_eq!(m.insert(0, 2), None);
        assert_eq!(m.insert(1, 3), None);
        assert_eq!(m.remove(&2), None);
        assert_eq!(m.remove(&1), Some(3));
        assert_eq!(m.remove(&0), Some(2));
    }

    #[test]
    fn clear() {
        let mut m = SmallMap::default();

        m.clear();
        assert_eq!(m.get(&0), None);

        m.insert(0, 1);
        m.clear();
        assert_eq!(m.get(&0), None);

        m.insert(0, 2);
        m.insert(1, 3);
        m.clear();
        assert_eq!(m.get(&0), None);
        assert_eq!(m.get(&1), None);
    }
}
