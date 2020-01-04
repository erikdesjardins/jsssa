use std::collections::HashMap;

use swc_atoms::JsWord;

use crate::collections::ZeroOneMany::{self, Many, One};
use crate::ir;
use crate::ir::traverse::Folder;
use crate::ir::F64;

/// Constant propagation / precompute.
///
/// Does not profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
#[derive(Debug, Default)]
pub struct ConstProp {
    shallow_values: HashMap<ir::WeakRef<ir::Ssa>, Expr>,
}

/// Cached expr with limited information.
#[derive(Debug)]
enum Expr {
    Bool(bool),
    Number(F64),
    String(JsWord),
    Null,
    Undefined,
    This,
    Array,
    Object,
    RegExp,
    Function,
    CurrentFunction,
    Argument,
}

impl ConstProp {
    fn maybe_shallow_cache_expr(&mut self, ref_: &ir::Ref<ir::Ssa>, expr: &ir::Expr) {
        // avoid cloning refs, which wastes time on refcounts and make tracing ref drops harder
        // and CERTAINLY avoid deep cloning blocks
        let shallow_clone = match expr {
            ir::Expr::Bool { value } => Expr::Bool(*value),
            ir::Expr::Number { value } => Expr::Number(*value),
            ir::Expr::String { value } => Expr::String(value.clone()),
            ir::Expr::Null => Expr::Null,
            ir::Expr::Undefined => Expr::Undefined,
            ir::Expr::This => Expr::This,
            // avoid cloning refs inside array
            ir::Expr::Array { elems: _ } => Expr::Array,
            // avoid cloning refs inside object
            ir::Expr::Object { props: _ } => Expr::Object,
            ir::Expr::RegExp { regex: _, flags: _ } => Expr::RegExp,
            // avoid cloning function body
            ir::Expr::Function { kind: _, body: _ } => Expr::Function,
            ir::Expr::CurrentFunction => Expr::CurrentFunction,
            ir::Expr::Argument { index: _ } => Expr::Argument,
            _ => return,
        };

        self.shallow_values.insert(ref_.weak(), shallow_clone);
    }
}

impl Folder for ConstProp {
    type Output = ZeroOneMany<ir::Stmt>;

    #[rustfmt::skip]
    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        use ir::BinaryOp::*;
        use ir::Expr::*;
        use ir::UnaryOp::*;

        match stmt {
            ir::Stmt::Expr {
                target,
                expr,
            } => {
                let expr = match expr {
                    ReadGlobal { ref source } => match source.as_ref() {
                        "NaN" => Number { value: F64::NAN },
                        "undefined" => Undefined,
                        _ => expr,
                    },
                    ReadMember { ref obj, ref prop } => match (self.shallow_values.get(&obj.weak()), self.shallow_values.get(&prop.weak())) {
                        (Some(obj), Some(Expr::String(prop))) => match (obj, prop.as_ref()) {
                            (Expr::String(value), "length") => Number { value: F64::from(value.chars().map(char::len_utf16).sum::<usize>() as f64) },
                            _ => expr,
                        },
                        _ => expr,
                    }
                    Unary {
                        ref op,
                        ref val,
                    } => match self.shallow_values.get(&val.weak()) {
                        Some(val_val) => match (op, val_val) {
                            (Plus, Expr::Number(value)) => Number { value: *value },
                            (Minus, Expr::Number(value)) => Number { value: -*value },
                            (Not, Expr::Bool(value)) => Bool { value: !*value },
                            (Not, Expr::Number(value)) => Bool { value: !value.is_truthy() },
                            (Not, Expr::String(value)) => Bool { value: value == "" },
                            (Not, Expr::Null)
                            | (Not, Expr::Undefined) => Bool { value: true },
                            (Not, Expr::Array)
                            | (Not, Expr::Object)
                            | (Not, Expr::RegExp)
                            | (Not, Expr::Function)
                            | (Not, Expr::CurrentFunction) => Bool { value: false },
                            (Tilde, Expr::Number(value)) => Number { value: !*value },
                            (Typeof, Expr::Bool(_)) => String { value: "boolean".into() },
                            (Typeof, Expr::Number(_)) => String { value: "number".into() },
                            (Typeof, Expr::String(_)) => String { value: "string".into() },
                            (Typeof, Expr::Null) => String { value: "object".into() },
                            (Typeof, Expr::Undefined) => String { value: "undefined".into() },
                            (Typeof, Expr::Array)
                            | (Typeof, Expr::Object)
                            | (Typeof, Expr::RegExp) => String { value: "object".into() },
                            (Typeof, Expr::Function)
                            | (Typeof, Expr::CurrentFunction) => String { value: "function".into() },
                            (Void, _) => Undefined,
                            _ => expr,
                        },
                        None => expr,
                    },
                    Binary {
                        ref op,
                        ref left,
                        ref right,
                    } => match (self.shallow_values.get(&left.weak()), self.shallow_values.get(&right.weak())) {
                        (Some(left_val), Some(right_val)) => match (op, left_val, right_val) {
                            (EqEq, _, _)
                            | (StrictEq, _, _) if left == right => Bool { value: true },
                            (EqEq, Expr::Bool(a), Expr::Bool(b))
                            | (StrictEq, Expr::Bool(a), Expr::Bool(b)) => Bool { value: a == b },
                            (EqEq, Expr::Number(a), Expr::Number(b))
                            | (StrictEq, Expr::Number(a), Expr::Number(b)) => Bool { value: a == b },
                            (EqEq, Expr::String(a), Expr::String(b))
                            | (StrictEq, Expr::String(a), Expr::String(b)) => Bool { value: a == b },
                            (NotEq, Expr::Bool(a), Expr::Bool(b))
                            | (NotStrictEq, Expr::Bool(a), Expr::Bool(b)) => Bool { value: a != b },
                            (NotEq, Expr::Number(a), Expr::Number(b))
                            | (NotStrictEq, Expr::Number(a), Expr::Number(b)) => Bool { value: a != b },
                            (NotEq, Expr::String(a), Expr::String(b))
                            | (NotStrictEq, Expr::String(a), Expr::String(b)) => Bool { value: a != b },
                            (Lt, Expr::Number(a), Expr::Number(b)) => Bool { value: a < b },
                            (LtEq, Expr::Number(a), Expr::Number(b)) => Bool { value: a <= b },
                            (Gt, Expr::Number(a), Expr::Number(b)) => Bool { value: a > b },
                            (GtEq, Expr::Number(a), Expr::Number(b)) => Bool { value: a >= b },
                            (ShiftLeft, Expr::Number(a), Expr::Number(b)) => Number { value: a.shl(*b) },
                            (ShiftRight, Expr::Number(a), Expr::Number(b)) => Number { value: a.shr(*b) },
                            (ShiftRightZero, Expr::Number(a), Expr::Number(b)) => Number { value: a.shr_zero(*b) },
                            (Add, Expr::Number(a), Expr::Number(b)) => Number { value: *a + *b },
                            (Sub, Expr::Number(a), Expr::Number(b)) => Number { value: *a - *b },
                            (Mul, Expr::Number(a), Expr::Number(b)) => Number { value: *a * *b },
                            (Div, Expr::Number(a), Expr::Number(b)) => Number { value: *a / *b },
                            (Mod, Expr::Number(a), Expr::Number(b)) => Number { value: *a % *b },
                            (BitOr, Expr::Number(a), Expr::Number(b)) => Number { value: *a | *b },
                            (BitXor, Expr::Number(a), Expr::Number(b)) => Number { value: *a ^ *b },
                            (BitAnd, Expr::Number(a), Expr::Number(b)) => Number { value: *a & *b },
                            (Exp, Expr::Number(a), Expr::Number(b)) => Number { value: a.powf(*b) },
                            (Add, Expr::String(a), Expr::String(b)) => String { value: (a.to_string() + b).into() },
                            _ => expr,
                        },
                        _ => expr,
                    },
                    _ => expr,
                };

                self.maybe_shallow_cache_expr(&target, &expr);

                One(ir::Stmt::Expr { target, expr })
            }
            ir::Stmt::IfElse {
                cond,
                cons,
                alt,
            } => match self.shallow_values.get(&cond.weak()) {
                Some(cond_val) => match cond_val {
                    Expr::Bool(true)
                    | Expr::Array
                    | Expr::Object
                    | Expr::RegExp
                    | Expr::Function => Many(cons.0),
                    Expr::Bool(false)
                    | Expr::Null
                    | Expr::Undefined => Many(alt.0),
                    Expr::Number(value) => if value.is_truthy() { Many(cons.0) } else { Many(alt.0) },
                    Expr::String(value) => if value != "" { Many(cons.0) } else { Many(alt.0) },
                    _ => One(ir::Stmt::IfElse { cond, cons, alt }),
                },
                None => One(ir::Stmt::IfElse { cond, cons, alt }),
            },
            _ => One(stmt),
        }
    }
}
