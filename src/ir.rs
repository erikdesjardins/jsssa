use std::f64;
use std::hash::{Hash, Hasher};

use swc_atoms::JsWord;

pub use self::print::print;
pub use self::ref_::{Lbl, Mut, Ref, RefType, Ssa, Used, WeakRef};

mod print;
mod ref_;
pub mod scope;
pub mod traverse;

#[derive(Debug, Hash)]
pub struct Block(pub Vec<Stmt>);

#[derive(Debug, Hash)]
pub enum Stmt {
    Expr {
        target: Ref<Ssa>,
        expr: Expr,
    },
    DeclareMutable {
        target: Ref<Mut>,
        val: Ref<Ssa>,
    },
    WriteMutable {
        target: Ref<Mut>,
        val: Ref<Ssa>,
    },
    WriteGlobal {
        target: JsWord,
        val: Ref<Ssa>,
    },
    WriteMember {
        obj: Ref<Ssa>,
        prop: Ref<Ssa>,
        val: Ref<Ssa>,
    },
    Return {
        val: Ref<Ssa>,
    },
    Throw {
        val: Ref<Ssa>,
    },
    Break {
        label: Option<Ref<Lbl>>,
    },
    Continue {
        label: Option<Ref<Lbl>>,
    },
    Debugger,
    Label {
        label: Ref<Lbl>,
        body: Block,
    },
    Loop {
        body: Block,
    },
    ForEach {
        kind: ForKind,
        init: Ref<Ssa>,
        body: Block,
    },
    IfElse {
        cond: Ref<Ssa>,
        cons: Block,
        alt: Block,
    },
    Switch {
        discr: Ref<Ssa>,
        body: Block,
    },
    SwitchCase {
        val: Option<Ref<Ssa>>,
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
    },
    Null,
    Undefined,
    This,
    Read {
        source: Ref<Ssa>,
    },
    ReadMutable {
        source: Ref<Mut>,
    },
    ReadGlobal {
        source: JsWord,
    },
    ReadMember {
        obj: Ref<Ssa>,
        prop: Ref<Ssa>,
    },
    Array {
        elems: Vec<Option<(EleKind, Ref<Ssa>)>>,
    },
    Object {
        props: Vec<(PropKind, Ref<Ssa>, Ref<Ssa>)>,
    },
    RegExp {
        regex: JsWord,
        flags: JsWord,
    },
    Unary {
        op: UnaryOp,
        val: Ref<Ssa>,
    },
    Binary {
        op: BinaryOp,
        left: Ref<Ssa>,
        right: Ref<Ssa>,
    },
    Delete {
        obj: Ref<Ssa>,
        prop: Ref<Ssa>,
    },
    Yield {
        kind: YieldKind,
        val: Ref<Ssa>,
    },
    Await {
        val: Ref<Ssa>,
    },
    Call {
        kind: CallKind,
        func: Ref<Ssa>,
        args: Vec<(EleKind, Ref<Ssa>)>,
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

#[derive(Debug, Clone, Hash)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    Tilde,
    Typeof,
    Void,
}

#[derive(Debug, Clone, Hash)]
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

#[derive(Debug, Clone, Hash)]
pub enum ForKind {
    In,
    Of,
}

#[derive(Debug, Clone, Hash)]
pub enum EleKind {
    Single,
    Spread,
}

#[derive(Debug, Clone, Hash)]
pub enum PropKind {
    Simple,
    Get,
    Set,
}

#[derive(Debug, Clone, Hash)]
pub enum CallKind {
    Call,
    New,
}

#[derive(Debug, Clone, Hash)]
pub enum FnKind {
    Func { is_async: bool, is_generator: bool },
    Arrow { is_async: bool },
}

#[derive(Debug, Clone, Hash)]
pub enum YieldKind {
    Single,
    Delegate,
}

/// f64 wrapper which allows hashing via NaN canonicalization
#[derive(Debug, Copy, Clone)]
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
