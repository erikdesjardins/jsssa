use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use swc_atoms::JsWord;

// references can either be SSA or mutable (needed to model closures)
pub trait RefType {}
impl RefType for SSA {}
impl RefType for Mutable {}
#[derive(Clone, Debug)]
pub enum SSA {}
#[derive(Clone, Debug)]
pub enum Mutable {}

pub enum Ref<T: RefType> {
    Dead,
    Live(LiveRef<T>),
}

impl<T: RefType> Clone for Ref<T> {
    fn clone(&self) -> Self {
        match self {
            Ref::Dead => Ref::Dead,
            Ref::Live(live_ref) => Ref::Live(LiveRef(live_ref.0.clone())),
        }
    }
}

impl<T: RefType> Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Ref::Dead => write!(f, "Ref(<dead>)"),
            Ref::Live(LiveRef(inner)) => write!(f, "Ref({} '{}')", inner.id, inner.name_hint),
        }
    }
}

impl<T: RefType> Ref<T> {
    pub fn new(name_hint: impl Into<JsWord>) -> Self {
        Ref::Live(LiveRef::new(name_hint.into()))
    }
}

pub struct LiveRef<T: RefType>(Rc<LiveRefInner<T>>);

struct LiveRefInner<T: RefType> {
    id: usize,
    name_hint: JsWord,
    _t: PhantomData<T>,
}

impl<T: RefType> PartialEq for LiveRef<T> {
    fn eq(&self, other: &Self) -> bool {
        // compare only by id, which is unique by construction
        self.0.id == other.0.id
    }
}

impl<T: RefType> LiveRef<T> {
    fn new(name_hint: JsWord) -> Self {
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
