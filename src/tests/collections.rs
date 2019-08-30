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
