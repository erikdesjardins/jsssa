use std::collections::HashMap;
use std::f64;

use crate::collections::ZeroOneMany::{self, Many, One, Zero};
use crate::ir;
use crate::ir::traverse::Folder;

/// Constant propagation / precompute.
///
/// Does not profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
#[derive(Default)]
pub struct ConstProp {
    shallow_values: HashMap<ir::WeakRef<ir::Ssa>, ir::Expr>,
}

impl ConstProp {
    fn maybe_shallow_cache_expr(&mut self, ref_: &ir::Ref<ir::Ssa>, expr: &ir::Expr) {
        // avoid cloning refs, which wastes time on refcounts and make tracing ref drops harder
        // and CERTAINLY avoid deep cloning blocks
        let shallow_clone = match expr {
            ir::Expr::Bool { value } => ir::Expr::Bool { value: *value },
            ir::Expr::Number { value } => ir::Expr::Number { value: *value },
            ir::Expr::String { value } => ir::Expr::String {
                value: value.clone(),
            },
            ir::Expr::Null => ir::Expr::Null,
            ir::Expr::Undefined => ir::Expr::Undefined,
            ir::Expr::This => ir::Expr::This,
            // avoid cloning refs inside array
            ir::Expr::Array { elems } if elems.is_empty() => ir::Expr::Array { elems: vec![] },
            // avoid cloning refs inside object
            ir::Expr::Object { props } if props.is_empty() => ir::Expr::Object { props: vec![] },
            ir::Expr::RegExp { regex, flags } => ir::Expr::RegExp {
                regex: regex.clone(),
                flags: flags.clone(),
            },
            // avoid cloning function body
            ir::Expr::Function { kind, body: _ } => ir::Expr::Function {
                kind: kind.clone(),
                body: ir::Block(vec![]),
            },
            ir::Expr::CurrentFunction => ir::Expr::CurrentFunction,
            ir::Expr::Argument { index } => ir::Expr::Argument { index: *index },
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
        use ir::F64;

        #[allow(clippy::cast_lossless, clippy::float_cmp)]
        match stmt {
            ir::Stmt::Expr {
                target,
                expr,
            } => {
                let expr = match expr {
                    ReadGlobal { ref source } if source == "NaN" => Number { value: F64(f64::NAN) },
                    ReadGlobal { ref source } if source == "undefined" => Undefined,
                    Unary {
                        ref op,
                        ref val,
                    } => match self.shallow_values.get(&val.weak()) {
                        Some(val_val) => match (op, val_val) {
                            (Plus, Number { value }) => Number { value: *value },
                            (Minus, Number { value }) => Number { value: F64(-value.0) },
                            (Not, Number { value }) => Bool { value: value.0 == 0.0 },
                            (Not, Bool { value }) => Bool { value: !*value },
                            (Tilde, Number { value }) => Number { value: F64(!(value.0 as i32) as f64) },
                            (Typeof, Bool { .. }) => String { value: "boolean".into() },
                            (Typeof, Number { .. }) => String { value: "number".into() },
                            (Typeof, String { .. }) => String { value: "string".into() },
                            (Typeof, Null) => String { value: "object".into() },
                            (Typeof, Undefined) => String { value: "undefined".into() },
                            (Typeof, Array { .. })
                            | (Typeof, Object { .. })
                            | (Typeof, RegExp { .. }) => String { value: "object".into() },
                            (Typeof, Function { .. })
                            | (Typeof, CurrentFunction) => String { value: "function".into() },
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
                            (EqEq, Bool { value: a }, Bool { value: b })
                            | (StrictEq, Bool { value: a }, Bool { value: b }) => Bool { value: a == b },
                            (EqEq, Number { value: a }, Number { value: b })
                            | (StrictEq, Number { value: a }, Number { value: b }) => Bool { value: a.0 == b.0 },
                            (NotEq, Bool { value: a }, Bool { value: b })
                            | (NotStrictEq, Bool { value: a }, Bool { value: b }) => Bool { value: a != b },
                            (NotEq, Number { value: a }, Number { value: b })
                            | (NotStrictEq, Number { value: a }, Number { value: b }) => Bool { value: a.0 != b.0 },
                            (Lt, Number { value: a }, Number { value: b }) => Bool { value: a.0 < b.0 },
                            (LtEq, Number { value: a }, Number { value: b }) => Bool { value: a.0 <= b.0 },
                            (Gt, Number { value: a }, Number { value: b }) => Bool { value: a.0 > b.0 },
                            (GtEq, Number { value: a }, Number { value: b }) => Bool { value: a.0 >= b.0 },
                            (ShiftLeft, Number { value: a }, Number { value: b }) => Number { value: F64(((a.0 as i32) << b.0 as i32) as f64) },
                            (ShiftRight, Number { value: a }, Number { value: b }) => Number { value: F64(((a.0 as i32) >> b.0 as i32) as f64) },
                            (ShiftRightZero, Number { value: a }, Number { value: b }) => Number { value: F64(((a.0 as i32 as u32) >> b.0 as i32) as f64) },
                            (Add, Number { value: a }, Number { value: b }) => Number { value: F64(a.0 + b.0) },
                            (Sub, Number { value: a }, Number { value: b }) => Number { value: F64(a.0 - b.0) },
                            (Mul, Number { value: a }, Number { value: b }) => Number { value: F64(a.0 * b.0) },
                            (Div, Number { value: a }, Number { value: b }) => Number { value: F64(a.0 / b.0) },
                            (Mod, Number { value: a }, Number { value: b }) => Number { value: F64(a.0 % b.0) },
                            (BitOr, Number { value: a }, Number { value: b }) => Number { value: F64((a.0 as i32 | b.0 as i32) as f64) },
                            (BitXor, Number { value: a }, Number { value: b }) => Number { value: F64((a.0 as i32 ^ b.0 as i32) as f64) },
                            (BitAnd, Number { value: a }, Number { value: b }) => Number { value: F64((a.0 as i32 & b.0 as i32) as f64) },
                            (Exp, Number { value: a }, Number { value: b }) => Number { value: F64(a.0.powf(b.0)) },
                            (Add, String { value: a }, String { value: b }) => String { value: (a.to_string() + b).into() },
                            _ => expr,
                        },
                        _ => expr,
                    },
                    _ => expr,
                };

                self.maybe_shallow_cache_expr(&target, &expr);

                One(ir::Stmt::Expr { target, expr })
            }
            ir::Stmt::ForEach {
                ref kind,
                ref init,
                body: _,
            } => match self.shallow_values.get(&init.weak()) {
                Some(init_val) => match (kind, init_val) {
                    (ir::ForKind::In, Object { props }) if props.is_empty() => Zero,
                    (ir::ForKind::Of, Array { elems }) if elems.is_empty() => Zero,
                    _ => One(stmt),
                },
                None => One(stmt),
            },
            ir::Stmt::IfElse {
                cond,
                cons,
                alt,
            } => match self.shallow_values.get(&cond.weak()) {
                Some(cond_val) => match cond_val {
                    Bool { value: true }
                    | Array { .. }
                    | Object { .. }
                    | RegExp { .. }
                    | Function { .. } => Many(cons.0),
                    Bool { value: false }
                    | Null
                    | Undefined => Many(alt.0),
                    Number { value } => if value.0 == 0. || value.0.is_nan() { Many(alt.0) } else { Many(cons.0) },
                    String { value } => if value == "" { Many(alt.0) } else { Many(cons.0) },
                    _ => One(ir::Stmt::IfElse { cond, cons, alt }),
                },
                None => One(ir::Stmt::IfElse { cond, cons, alt }),
            },
            _ => One(stmt),
        }
    }
}
