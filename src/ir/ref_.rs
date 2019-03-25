use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
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

pub struct Ref<T: RefType>(Rc<RefInner<T>>);

struct RefInner<T: RefType> {
    id: usize,
    name_hint: JsWord,
    _t: PhantomData<T>,
}

impl<T: RefType> Ref<T> {
    pub fn new(name_hint: impl Into<JsWord>) -> Self {
        static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

        Ref(Rc::new(RefInner {
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            name_hint: name_hint.into(),
            _t: PhantomData,
        }))
    }

    pub fn dead() -> Self {
        Self::new("")
    }

    pub fn name_hint(&self) -> &JsWord {
        &self.0.name_hint
    }

    pub fn maybe_used(&self) -> bool {
        Rc::strong_count(&self.0) > 1
    }
}

impl<T: RefType> Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.maybe_used() {
            write!(f, "Ref({} '{}')", self.0.id, self.0.name_hint)
        } else {
            write!(f, "Ref(<dead>)")
        }
    }
}

impl<T: RefType> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: RefType> Eq for Ref<T> {}

impl<T: RefType> PartialEq for Ref<T> {
    fn eq(&self, other: &Self) -> bool {
        // compare only by id, which is unique by construction
        self.0.id == other.0.id
    }
}

impl<T: RefType> Hash for Ref<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.0.id)
    }
}
