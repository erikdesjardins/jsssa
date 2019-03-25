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

// todo allow inlined IR via Stmt<T> { ... val: T } and ssa ir is Stmt<Ref<SSA>>
#[derive(Debug)]
pub enum Stmt {
    Expr {
        target: Ref<SSA>,
        expr: Expr,
    },
    DeclareMutable {
        target: Ref<Mutable>,
        val: Ref<SSA>,
    },
    WriteMutable {
        target: Ref<Mutable>,
        val: Ref<SSA>,
    },
    WriteGlobal {
        target: JsWord,
        val: Ref<SSA>,
    },
    WriteMember {
        obj: Ref<SSA>,
        prop: Ref<SSA>,
        val: Ref<SSA>,
    },
    Return {
        val: Ref<SSA>,
    },
    Throw {
        val: Ref<SSA>,
    },
    Break,
    Continue,
    Debugger,
    Block {
        body: Box<Block>,
    },
    Loop {
        body: Box<Block>,
    },
    ForEach {
        kind: ForKind,
        init: Ref<SSA>,
        body: Box<Block>,
    },
    IfElse {
        cond: Ref<SSA>,
        cons: Box<Block>,
        alt: Box<Block>,
    },
    Try {
        body: Box<Block>,
        catch: Option<Box<Block>>,
        finally: Option<Box<Block>>,
    },
}

#[derive(Debug)]
pub enum Expr {
    Bool {
        value: bool,
    },
    Number {
        value: f64,
    },
    String {
        value: JsWord,
        has_escape: bool,
    },
    Null,
    Undefined,
    This,
    Read {
        source: Ref<SSA>,
    },
    ReadMutable {
        source: Ref<Mutable>,
    },
    ReadGlobal {
        source: JsWord,
    },
    ReadMember {
        obj: Ref<SSA>,
        prop: Ref<SSA>,
    },
    Array {
        elems: Vec<Option<(EleKind, Ref<SSA>)>>,
    },
    Object {
        props: Vec<(PropKind, Ref<SSA>, Ref<SSA>)>,
    },
    RegExp {
        regex: JsWord,
        has_escape: bool,
        flags: Option<JsWord>,
    },
    Unary {
        op: UnaryOp,
        val: Ref<SSA>,
    },
    Binary {
        op: BinaryOp,
        left: Ref<SSA>,
        right: Ref<SSA>,
    },
    Delete {
        obj: Ref<SSA>,
        prop: Ref<SSA>,
    },
    Yield {
        kind: YieldKind,
        val: Ref<SSA>,
    },
    Await {
        val: Ref<SSA>,
    },
    Call {
        kind: CallKind,
        func: Ref<SSA>,
        args: Vec<(EleKind, Ref<SSA>)>,
    },
    Function {
        kind: FnKind,
        body: Box<Block>,
    },
    CurrentFunction,
    Argument {
        index: usize,
    },
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
    LtEq,
    Gt,
    GtEq,
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
