use std::collections::HashMap;

use crate::collections::ZeroOneMany::{self, Many, One, Zero};
use crate::ir;
use crate::ir::fold::Folder;

/// Constant propagation / precompute.
///
/// Does not profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
#[derive(Default)]
pub struct Prop {
    known_values: HashMap<ir::Ref<ir::Ssa>, ir::Expr>,
}

impl Folder for Prop {
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
                    Unary {
                        ref op,
                        ref val,
                    } => match (op, &self.known_values[val]) {
                        (Plus, Number { value }) => Number { value: *value },
                        (Minus, Number { value }) => Number { value: F64(-value.0) },
                        (Not, Number { value }) => Bool { value: value.0 == 0.0 },
                        (Not, Bool { value }) => Bool { value: !*value },
                        (Tilde, Number { value }) => Number { value: F64(!(value.0 as i32) as f64) },
                        (Typeof, Bool { .. }) => String { value: "boolean".into(), has_escape: false },
                        (Typeof, Number { .. }) => String { value: "number".into(), has_escape: false },
                        (Typeof, String { .. }) => String { value: "string".into(), has_escape: false },
                        (Typeof, Null) => String { value: "object".into(), has_escape: false },
                        (Typeof, Undefined) => String { value: "undefined".into(), has_escape: false },
                        (Typeof, Array { .. })
                        | (Typeof, Object { .. })
                        | (Typeof, RegExp { .. }) => String { value: "object".into(), has_escape: false },
                        (Typeof, Function { .. }) => String { value: "function".into(), has_escape: false },
                        (Void, _) => Undefined,
                        _ => expr,
                    },
                    Binary {
                        ref op,
                        ref left,
                        ref right,
                    } => match (op, &self.known_values[left], &self.known_values[right]) {
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
                        (Add, String { value: a, has_escape: a_escape }, String { value: b, has_escape: b_escape })
                        => String { value: (a.to_string() + b).into(), has_escape: *a_escape || *b_escape },
                        _ => expr,
                    },
                    _ => expr,
                };
                self.known_values.insert(target.clone(), expr.clone());
                One(ir::Stmt::Expr { target, expr })
            }
            ir::Stmt::ForEach {
                ref kind,
                ref init,
                body: _,
            } => match (kind, &self.known_values[init]) {
                (ir::ForKind::In, Object { props }) if props.is_empty() => Zero,
                (ir::ForKind::Of, Array { elems }) if elems.is_empty() => Zero,
                _ => One(stmt),
            },
            ir::Stmt::IfElse {
                cond,
                cons,
                alt,
            } => match &self.known_values[&cond] {
                Bool { value: true }
                | Array { .. }
                | Object { .. }
                | RegExp { .. }
                | Function { .. } => Many(cons.0),
                Bool { value: false }
                | Null
                | Undefined => Many(alt.0),
                Number { value } => if value.0 == 0. || value.0.is_nan() { Many(alt.0) } else { Many(cons.0) },
                String { value, has_escape: _ } => if value == "" { Many(alt.0) } else { Many(cons.0) },
                _ => One(ir::Stmt::IfElse { cond, cons, alt }),
            },
            _ => One(stmt),
        }
    }
}
