use swc_atoms::JsWord;

pub use self::ref_::{Mutable, Ref, RefType, SSA};

mod ref_;
pub mod scope;

#[derive(Debug)]
pub struct Block {
    pub children: Vec<Stmt>,
}

impl Block {
    pub fn empty() -> Self {
        Self { children: vec![] }
    }

    pub fn with_children(children: Vec<Stmt>) -> Self {
        Self { children }
    }
}

#[derive(Debug)]
pub enum Stmt {
    Expr(Expr),
    WriteSsa(Ref<SSA>, Expr),
    WriteBinding(Ref<Mutable>, Ref<SSA>),
    WriteGlobal(JsWord, Ref<SSA>),
    WriteMember(Ref<SSA>, Ref<SSA>, Ref<SSA>),
    Return(Ref<SSA>),
    Throw(Ref<SSA>),
    Break,
    Continue,
    Debugger,
    Block(Box<Block>),
    Loop(Box<Block>),
    For(ForKind, Ref<Mutable>, Ref<SSA>, Box<Block>),
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
    String(JsWord),
    Null,
    Undefined,
    This,
    Super,
    Read(Ref<SSA>),
    ReadBinding(Ref<Mutable>),
    ReadGlobal(JsWord),
    ReadMember(Ref<SSA>, Ref<SSA>),
    Array(Vec<Option<(EleKind, Ref<SSA>)>>),
    Object(Vec<(PropKind, Ref<SSA>, Ref<SSA>)>),
    RegExp(JsWord, Option<JsWord>),
    Unary(UnaryOp, Ref<SSA>),
    Binary(BinaryOp, Ref<SSA>, Ref<SSA>),
    Delete(Ref<SSA>, Ref<SSA>),
    Call(CallKind, Ref<SSA>, Vec<(EleKind, Ref<SSA>)>),
    Function(FnKind, Option<JsWord>, Vec<Ref<Mutable>>, Box<Block>),
    CurrentFunction,
    Yield(YieldKind, Ref<SSA>),
    Await(Ref<SSA>),
}

#[derive(Debug)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    Tilde,
    Typeof,
    Void,
}

#[derive(Debug)]
pub enum BinaryOp {
    EqEq,
    NotEq,
    StrictEq,
    NotStrictEq,
    Lt,
    Lte,
    Gt,
    Gte,
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
    Exp,
    In,
    Instanceof,
}

#[derive(Debug)]
pub enum ForKind {
    In,
    Of,
}

#[derive(Debug)]
pub enum EleKind {
    Single,
    Spread,
}

#[derive(Debug)]
pub enum PropKind {
    Simple,
    Get,
    Set,
}

#[derive(Debug)]
pub enum CallKind {
    Call,
    New,
}

#[derive(Debug)]
pub enum FnKind {
    Func { is_async: bool, is_generator: bool },
    Arrow { is_async: bool },
}

#[derive(Debug)]
pub enum YieldKind {
    Single,
    Delegate,
}
