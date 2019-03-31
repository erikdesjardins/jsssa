use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use swc_atoms::JsWord;

pub trait RefType {}
impl RefType for SSA {}
impl RefType for Mutable {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SSA {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mutable {}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Ref<T: RefType> {
    inner: Rc<RefInner<T>>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WeakRef<T: RefType> {
    id: usize,
    _t: PhantomData<T>,
}

struct RefInner<T: RefType> {
    id: usize,
    name_hint: JsWord,
    _t: PhantomData<T>,
}

impl<T: RefType> Ref<T> {
    pub fn new(name_hint: impl Into<JsWord>) -> Self {
        static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

        Self {
            inner: Rc::new(RefInner {
                id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
                name_hint: name_hint.into(),
                _t: PhantomData,
            }),
        }
    }

    pub fn dead() -> Self {
        Self::new("_")
    }

    pub fn name_hint(&self) -> &JsWord {
        &self.inner.name_hint
    }

    pub fn used(&self) -> Used {
        match Rc::strong_count(&self.inner) {
            0 /* impossible */ | 1 => Used::Never,
            2 => Used::Once,
            _ => Used::Mult,
        }
    }

    pub fn weak(&self) -> WeakRef<T> {
        WeakRef {
            id: self.inner.id,
            _t: PhantomData,
        }
    }
}

impl<T: RefType> Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.used().is_never() {
            write!(f, "Ref(<dead>)")
        } else {
            write!(f, "Ref({} '{}')", self.inner.id, self.inner.name_hint)
        }
    }
}

impl<T: RefType> Debug for WeakRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "WeakRef({})", self.id)
    }
}

impl<T: RefType> PartialEq for RefInner<T> {
    fn eq(&self, other: &Self) -> bool {
        // compare only by id, which is unique by construction
        self.id == other.id
    }
}

impl<T: RefType> Eq for RefInner<T> {}

impl<T: RefType> Hash for RefInner<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.id)
    }
}

pub enum Used {
    Never,
    Once,
    Mult,
}

impl Used {
    pub fn is_never(&self) -> bool {
        match self {
            Used::Never => true,
            _ => false,
        }
    }
}
