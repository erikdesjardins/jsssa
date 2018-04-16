use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

// references can either be SSA or mutable (needed to model closures)
pub trait RefType {}
impl RefType for SSA {}
impl RefType for Mutable {}
#[derive(Clone, Debug)]
pub enum SSA {}
#[derive(Clone, Debug)]
pub enum Mutable {}

#[derive(Clone)]
pub enum Ref<T: RefType> {
    Dead,
    Live(LiveRef<T>),
}

impl<T: RefType> Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut f = f.debug_tuple("Ref");
        match self {
            Ref::Dead => {
                f.field(&"Dead");
            }
            Ref::Live(LiveRef(inner)) => {
                f.field(&inner.id);
                f.field(&inner.name_hint);
            }
        }
        f.finish()
    }
}

impl<T: RefType> Ref<T> {
    pub fn new(name_hint: String) -> Self {
        Ref::Live(LiveRef::new(name_hint))
    }
}

#[derive(Clone)]
pub struct LiveRef<T: RefType>(Rc<LiveRefInner<T>>);

struct LiveRefInner<T: RefType> {
    id: usize,
    name_hint: String,
    _t: PhantomData<T>,
}

impl<T: RefType> PartialEq for LiveRef<T> {
    fn eq(&self, other: &Self) -> bool {
        // compare only by id, which is unique by construction
        self.0.id == other.0.id
    }
}

impl<T: RefType> LiveRef<T> {
    fn new(name_hint: String) -> Self {
        static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

        LiveRef(Rc::new(LiveRefInner {
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            name_hint,
            _t: PhantomData,
        }))
    }

    pub fn name_hint(&self) -> &str {
        &self.0.name_hint
    }

    pub fn might_be_used(&self) -> bool {
        Rc::strong_count(&self.0) > 1
    }
}
