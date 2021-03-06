use std::collections::{HashMap, HashSet};
use std::mem;

use crate::anl;
use crate::collections::ZeroOneMany::{self, Many, One, Zero};
use crate::ir;
use crate::ir::traverse::{Folder, ScopeTy};

/// Loop unrolling for 0/1 iteration loops.
///
/// Does not profit from multiple passes.
/// May profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for read forwarding.
/// May create opportunities for SROA.
#[derive(Debug, Default)]
pub struct Loops {
    refs_used_in_only_one_fn_scope: HashSet<ir::WeakRef<ir::Ssa>>,
    local: Objects,
    nonlocal: Objects,
}

#[derive(Debug, Default)]
struct Objects {
    small_objects: HashMap<ir::WeakRef<ir::Ssa>, Option<ir::Ref<ir::Ssa>>>,
    invalid_in_parent: Invalid,
}

#[derive(Debug)]
enum Invalid {
    All,
    Objects(HashSet<ir::WeakRef<ir::Ssa>>),
}

impl Default for Invalid {
    fn default() -> Self {
        Self::Objects(Default::default())
    }
}

impl Invalid {
    fn insert_obj(&mut self, obj: ir::WeakRef<ir::Ssa>) {
        match self {
            Self::All => {
                // all objects already invalid
            }
            Self::Objects(objects) => {
                objects.insert(obj);
            }
        }
    }
}

impl Loops {
    fn invalidate_from_child(&mut self, child: Self) {
        fn invalidate(this: &mut Objects, child: Objects) {
            match child.invalid_in_parent {
                Invalid::All => {
                    this.small_objects.clear();
                    this.invalid_in_parent = Invalid::All;
                }
                Invalid::Objects(objects) => {
                    for obj in objects {
                        this.small_objects.remove(&obj);
                        this.invalid_in_parent.insert_obj(obj);
                    }
                }
            }
        }
        invalidate(&mut self.local, child.local);
        invalidate(&mut self.nonlocal, child.nonlocal);
    }

    fn invalidate_everything_used_across_fn_scopes(&mut self) {
        self.nonlocal.small_objects.clear();
        self.nonlocal.invalid_in_parent = Invalid::All;
    }

    fn invalidate_ref(&mut self, ref_: ir::WeakRef<ir::Ssa>) {
        let objects = if self.refs_used_in_only_one_fn_scope.contains(&ref_) {
            &mut self.local
        } else {
            &mut self.nonlocal
        };
        objects.small_objects.remove(&ref_);
        objects.invalid_in_parent.insert_obj(ref_);
    }

    fn declare_small_object(&mut self, ref_: ir::WeakRef<ir::Ssa>, prop: Option<ir::Ref<ir::Ssa>>) {
        let objects = if self.refs_used_in_only_one_fn_scope.contains(&ref_) {
            &mut self.local
        } else {
            &mut self.nonlocal
        };
        objects.small_objects.insert(ref_, prop);
    }

    fn get_small_object(&self, ref_: &ir::WeakRef<ir::Ssa>) -> Option<&Option<ir::Ref<ir::Ssa>>> {
        self.local
            .small_objects
            .get(ref_)
            .or_else(|| self.nonlocal.small_objects.get(ref_))
    }
}

impl Folder for Loops {
    type Output = ZeroOneMany<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            self.refs_used_in_only_one_fn_scope = anl::refs::used_in_only_one_fn_scope(&block)
                .map(ir::Ref::weak)
                .collect();
        }

        match ty {
            ScopeTy::Function => {
                // functions are analyzed separately
                let mut inner = Self::default();
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                let r = enter(&mut inner, block);
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel => {
                // we deal only with ssa refs, so it doesn't matter if usage info escapes normal scopes,
                // since there can be no uses of an ssa ref in a parent scope
                enter(self, block)
            }
            ScopeTy::Nonlinear => {
                // no information can be carried into a nonlinear scope, but invalidations must be applied
                let mut inner = Self::default();
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                let r = enter(&mut inner, block);
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                self.invalidate_from_child(inner);
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
                        self.declare_small_object(target.weak(), maybe_key);
                    }
                    One(stmt)
                }
                ir::Expr::Yield { .. } | ir::Expr::Await { .. } | ir::Expr::Call { .. } => {
                    self.invalidate_everything_used_across_fn_scopes();
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
            } => match self.get_small_object(&init.weak()) {
                Some(maybe_key) => match maybe_key {
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
                None => One(ir::Stmt::ForEach { kind, init, body }),
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
        self.invalidate_ref(ref_.weak());
        ref_
    }
}
