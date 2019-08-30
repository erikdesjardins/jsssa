use std::collections::HashMap;

use crate::collections::InvertibleSet;
use crate::collections::ZeroOneMany::{self, Many, One, Zero};
use crate::ir;
use crate::ir::traverse::{Folder, ScopeTy};

/// Loop unrolling for 0/1 iteration loops.
///
/// Does not profit from multiple passes.
/// May profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for read forwarding.
/// May create opportunities for SROA.
#[derive(Default)]
pub struct Loops {
    known_small_objects: HashMap<ir::WeakRef<ir::Ssa>, Option<ir::Ref<ir::Ssa>>>,
    invalid_objects: InvertibleSet<ir::WeakRef<ir::Ssa>>,
}

impl Folder for Loops {
    type Output = ZeroOneMany<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                // functions are analyzed separately
                let mut inner = Self::default();
                enter(&mut inner, block)
            }
            ScopeTy::Normal | ScopeTy::Toplevel => {
                // we deal only with ssa refs, so it doesn't matter if usage info escapes normal scopes,
                // since there can be no uses of an ssa ref in a parent scope
                enter(self, block)
            }
            ScopeTy::Nonlinear => {
                // no information can be carried into a nonlinear scope, but invalidations must be applied
                let mut inner = Self::default();
                let r = enter(&mut inner, block);
                self.invalid_objects.union_with(inner.invalid_objects);
                r
            }
        }
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                ref target,
                ref expr,
            } => match expr {
                ir::Expr::Object { props } => {
                    let mut props = props.iter();
                    if let (maybe_first, None) = (props.next(), props.next()) {
                        let maybe_key = maybe_first.map(|(_kind, key, _val)| key.clone());
                        self.known_small_objects.insert(target.weak(), maybe_key);
                    }
                    One(stmt)
                }
                ir::Expr::Yield { .. } | ir::Expr::Await { .. } | ir::Expr::Call { .. } => {
                    self.invalid_objects.insert_everything_except(vec![]);
                    One(stmt)
                }
                ir::Expr::Bool { .. }
                | ir::Expr::Number { .. }
                | ir::Expr::String { .. }
                | ir::Expr::Null
                | ir::Expr::Undefined
                | ir::Expr::This
                | ir::Expr::Read { .. }
                | ir::Expr::ReadMutable { .. }
                | ir::Expr::ReadGlobal { .. }
                | ir::Expr::ReadMember { .. }
                | ir::Expr::Array { .. }
                | ir::Expr::RegExp { .. }
                | ir::Expr::Unary { .. }
                | ir::Expr::Binary { .. }
                | ir::Expr::Delete { .. }
                | ir::Expr::Function { .. }
                | ir::Expr::CurrentFunction { .. }
                | ir::Expr::Argument { .. } => One(stmt),
            },
            ir::Stmt::ForEach {
                kind: kind @ ir::ForKind::In,
                init,
                body,
            } => match (
                self.invalid_objects.contains(&init.weak()),
                self.known_small_objects.get(&init.weak()),
            ) {
                (false, Some(maybe_key)) => match maybe_key {
                    Some(single_key) => {
                        let ir::Block(children) = body;
                        let single_iteration = children
                            .into_iter()
                            .map(|stmt| match stmt {
                                ir::Stmt::Expr {
                                    target,
                                    expr: ir::Expr::Argument { index: 0 },
                                } => ir::Stmt::Expr {
                                    target,
                                    expr: ir::Expr::Read {
                                        source: single_key.clone(),
                                    },
                                },
                                _ => stmt,
                            })
                            .collect();
                        Many(single_iteration)
                    }
                    None => Zero,
                },
                _ => One(ir::Stmt::ForEach { kind, init, body }),
            },
            ir::Stmt::DeclareMutable { .. }
            | ir::Stmt::WriteMutable { .. }
            | ir::Stmt::WriteGlobal { .. }
            | ir::Stmt::WriteMember { .. }
            | ir::Stmt::Return { .. }
            | ir::Stmt::Throw { .. }
            | ir::Stmt::Break { .. }
            | ir::Stmt::Continue { .. }
            | ir::Stmt::Debugger
            | ir::Stmt::Label { .. }
            | ir::Stmt::Loop { .. }
            | ir::Stmt::ForEach { .. }
            | ir::Stmt::IfElse { .. }
            | ir::Stmt::Switch { .. }
            | ir::Stmt::SwitchCase { .. }
            | ir::Stmt::Try { .. } => One(stmt),
        }
    }

    fn fold_ref_use(&mut self, ref_: ir::Ref<ir::Ssa>) -> ir::Ref<ir::Ssa> {
        // todo inefficient: this invalidates every single ref, even if it's not an object
        self.invalid_objects.insert(ref_.weak());
        ref_
    }
}
