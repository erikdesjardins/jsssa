use std::f64;
use std::hash::{Hash, Hasher};

use swc_atoms::JsWord;

pub use self::print::print;
pub use self::ref_::{Mutable, Ref, RefType, SSA};

pub mod fold;
mod print;
mod ref_;
pub mod scope;
pub mod visit;

#[derive(Debug, Hash)]
pub struct Block(pub Vec<Stmt>);

#[derive(Debug, Hash)]
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
    Loop {
        body: Block,
    },
    ForEach {
        kind: ForKind,
        init: Ref<SSA>,
        body: Block,
    },
    IfElse {
        cond: Ref<SSA>,
        cons: Block,
        alt: Block,
    },
    Try {
        body: Block,
        catch: Block,
        finally: Box<Block>,
    },
}

#[derive(Debug, Hash)]
pub enum Expr {
    Bool {
        value: bool,
    },
    Number {
        value: F64,
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
        flags: JsWord,
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
        body: Block,
    },
    CurrentFunction,
    Argument {
        index: usize,
    },
}

#[derive(Debug, Hash)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    Tilde,
    Typeof,
    Void,
}

#[derive(Debug, Hash)]
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

#[derive(Debug, Hash)]
pub enum ForKind {
    In,
    Of,
}

#[derive(Debug, Hash)]
pub enum EleKind {
    Single,
    Spread,
}

#[derive(Debug, Hash)]
pub enum PropKind {
    Simple,
    Get,
    Set,
}

#[derive(Debug, Hash)]
pub enum CallKind {
    Call,
    New,
}

#[derive(Debug, Hash)]
pub enum FnKind {
    Func { is_async: bool, is_generator: bool },
    Arrow { is_async: bool },
}

#[derive(Debug, Hash)]
pub enum YieldKind {
    Single,
    Delegate,
}

/// f64 wrapper which allows hashing via NaN canonicalization
#[derive(Debug)]
pub struct F64(pub f64);

impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.0.is_nan() {
            // use one specific NaN representation
            state.write_u64(f64::NAN.to_bits());
        } else {
            state.write_u64(self.0.to_bits());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stmt_is_512_bits() {
        assert_eq!(std::mem::size_of::<Stmt>(), 64);
    }
}
