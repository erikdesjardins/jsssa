use ir::*;

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
    Break,
    Continue,
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
    Super,
    Read(Ref<SSA>),
    ReadBinding(Ref<Mutable>),
    ReadGlobal(String),
    Array(Vec<Option<(EleKind, Ref<SSA>)>>),
    Object(Vec<(PropKind, Ref<SSA>, Ref<SSA>)>),
    RegExp(String, String),
    Unary(UnaryOp, Ref<SSA>),
    Delete(Ref<SSA>, Ref<SSA>),
    Binary(BinaryOp, Ref<SSA>, Ref<SSA>),
    Call(CallKind, Ref<SSA>, Vec<Ref<SSA>>),
    Function(FnKind, Option<String>, Vec<Ref<Mutable>>, Box<Block>),
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
    Func { async: bool, gen: bool },
    Arrow { async: bool },
}

#[derive(Debug)]
pub enum YieldKind {
    Single,
    Delegate,
}
