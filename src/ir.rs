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

#[derive(Clone, PartialEq)]
pub struct Ref<T: RefType>(Rc<RefInner<T>>);

impl<T: RefType> Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_tuple("Ref")
            .field(&self.0.id)
            .field(&self.0.name_hint)
            .finish()
    }
}

#[derive(PartialEq)]
struct RefInner<T: RefType> {
    id: usize,
    name_hint: String,
    _t: PhantomData<T>,
}

impl<T: RefType> Ref<T> {
    pub fn new(name_hint: String) -> Self {
        static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

        Ref(Rc::new(RefInner {
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

#[derive(Debug)]
pub struct Block {
    pub directives: Vec<String>,
    pub children: Vec<Stmt>,
}

impl Block {
    pub fn empty() -> Self {
        Block {
            directives: vec![],
            children: vec![],
        }
    }
}

#[derive(Debug)]
pub enum Stmt {
    Expr(Expr),
    Assign(Ref<SSA>, Expr),
    AssignBinding(Ref<Mutable>, Ref<SSA>),
    AssignGlobal(String, Ref<SSA>),
    Return(Ref<SSA>),
    Throw(Ref<SSA>),
    Debugger,
    Block(Box<Block>),
    IfElse(Ref<SSA>, Box<Block>, Box<Block>),
    Try(
        Box<Block>,
        Option<(Ref<Mutable>, Box<Block>)>,
        Option<Box<Block>>,
    ),
}

#[derive(Debug)]
pub enum Expr {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
    Undefined,
    This,
    Read(Ref<SSA>),
    ReadBinding(Ref<Mutable>),
    ReadGlobal(String),
    Array(Vec<Option<Ref<SSA>>>),
    Object(Vec<(Ref<SSA>, Ref<SSA>)>),
    RegExp(String, String),
    Unary(UnaryOp, Ref<SSA>),
    Binary(BinaryOp, Ref<SSA>, Ref<SSA>),
    Call(CallKind, Ref<SSA>, Vec<Ref<SSA>>),
    Function(FnKind, Option<String>, Vec<Ref<Mutable>>, Box<Block>),
    Yield(YieldKind, Ref<SSA>),
}

#[derive(Debug)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    Tilde,
    Typeof,
    Void,
    Delete,
}

#[derive(Debug)]
pub enum BinaryOp {
    Eq,
    NotEq,
    StrictEq,
    NotStrictEq,
    LT,
    LTE,
    GT,
    GTE,
    ShiftLeft,
    ShiftRight,
    ShiftRightZero,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitOr,
    BitXor,
    BitAnd,
    In,
    Instanceof,
}

#[derive(Debug)]
pub enum CallKind {
    Call,
    New,
}

#[derive(Debug)]
pub enum FnKind {
    Func { async: bool, gen: bool },
    Arrow { async: bool },
}

#[derive(Debug)]
pub enum YieldKind {
    Single,
    Delegate,
}
