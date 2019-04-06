use std::collections::HashMap;
use std::iter;

use crate::ir;
use crate::ir::traverse::{visit_with, Folder, ScopeTy};

/// Forward `ir::Expr::Read` to the source SSA ref.
///
/// Does not profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
#[derive(Default)]
pub struct Reads {
    ssa_remappings: HashMap<ir::WeakRef<ir::Ssa>, ir::Ref<ir::Ssa>>,
}

impl Reads {
    fn remap(&self, ref_: ir::Ref<ir::Ssa>) -> ir::Ref<ir::Ssa> {
        match self.ssa_remappings.get(&ref_.weak()) {
            Some(orig_ref) => orig_ref.clone(),
            None => ref_,
        }
    }
}

impl Folder for Reads {
    type Output = iter::Once<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            visit_with(&block, |stmt: &ir::Stmt| match stmt {
                ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Read { source },
                } => match self.ssa_remappings.get(&source.weak()) {
                    Some(orig_ref) => {
                        self.ssa_remappings.insert(target.weak(), orig_ref.clone());
                    }
                    None => {
                        self.ssa_remappings.insert(target.weak(), source.clone());
                    }
                },
                _ => {}
            });
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        iter::once(match stmt {
            ir::Stmt::Expr { target, expr } => ir::Stmt::Expr {
                target,
                expr: match expr {
                    ir::Expr::Read { source } => ir::Expr::Read {
                        source: self.remap(source),
                    },
                    ir::Expr::ReadMember { obj, prop } => ir::Expr::ReadMember {
                        obj: self.remap(obj),
                        prop: self.remap(prop),
                    },
                    ir::Expr::Array { elems } => ir::Expr::Array {
                        elems: elems
                            .into_iter()
                            .map(|opt| opt.map(|(kind, ele)| (kind, self.remap(ele))))
                            .collect(),
                    },
                    ir::Expr::Object { props } => ir::Expr::Object {
                        props: props
                            .into_iter()
                            .map(|(kind, obj, prop)| (kind, self.remap(obj), self.remap(prop)))
                            .collect(),
                    },
                    ir::Expr::Unary { op, val } => ir::Expr::Unary {
                        op,
                        val: self.remap(val),
                    },
                    ir::Expr::Binary { op, left, right } => ir::Expr::Binary {
                        op,
                        left: self.remap(left),
                        right: self.remap(right),
                    },
                    ir::Expr::Delete { obj, prop } => ir::Expr::Delete {
                        obj: self.remap(obj),
                        prop: self.remap(prop),
                    },
                    ir::Expr::Yield { kind, val } => ir::Expr::Yield {
                        kind,
                        val: self.remap(val),
                    },
                    ir::Expr::Await { val } => ir::Expr::Await {
                        val: self.remap(val),
                    },
                    ir::Expr::Call { kind, func, args } => ir::Expr::Call {
                        kind,
                        func: self.remap(func),
                        args: args
                            .into_iter()
                            .map(|(kind, arg)| (kind, self.remap(arg)))
                            .collect(),
                    },
                    ir::Expr::Bool { .. }
                    | ir::Expr::Number { .. }
                    | ir::Expr::String { .. }
                    | ir::Expr::Null
                    | ir::Expr::Undefined
                    | ir::Expr::This
                    | ir::Expr::ReadMutable { .. }
                    | ir::Expr::ReadGlobal { .. }
                    | ir::Expr::RegExp { .. }
                    | ir::Expr::Function { .. }
                    | ir::Expr::CurrentFunction
                    | ir::Expr::Argument { .. } => expr,
                },
            },
            ir::Stmt::DeclareMutable { target, val } => ir::Stmt::DeclareMutable {
                target,
                val: self.remap(val),
            },
            ir::Stmt::WriteMutable { target, val } => ir::Stmt::WriteMutable {
                target,
                val: self.remap(val),
            },
            ir::Stmt::WriteGlobal { target, val } => ir::Stmt::WriteGlobal {
                target,
                val: self.remap(val),
            },
            ir::Stmt::WriteMember { obj, prop, val } => ir::Stmt::WriteMember {
                obj: self.remap(obj),
                prop: self.remap(prop),
                val: self.remap(val),
            },
            ir::Stmt::Return { val } => ir::Stmt::Return {
                val: self.remap(val),
            },
            ir::Stmt::Throw { val } => ir::Stmt::Throw {
                val: self.remap(val),
            },
            ir::Stmt::ForEach { kind, init, body } => ir::Stmt::ForEach {
                kind,
                init: self.remap(init),
                body,
            },
            ir::Stmt::IfElse { cond, cons, alt } => ir::Stmt::IfElse {
                cond: self.remap(cond),
                cons,
                alt,
            },
            ir::Stmt::Switch { discr, body } => ir::Stmt::Switch {
                discr: self.remap(discr),
                body,
            },
            ir::Stmt::Break { .. }
            | ir::Stmt::Continue { .. }
            | ir::Stmt::Debugger
            | ir::Stmt::Label { .. }
            | ir::Stmt::Loop { .. }
            | ir::Stmt::SwitchCase { .. }
            | ir::Stmt::Try { .. } => stmt,
        })
    }
}
