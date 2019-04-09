use std::collections::{HashMap, HashSet};
use std::mem;

use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Converts read-only mutable vars to SSA, and removes write-only mutable vars.
///
/// Does not profit from multiple passes.
/// May profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for read forwarding.
#[derive(Default)]
pub struct Mut2Ssa {
    mut_vars_to_replace: HashMap<ir::WeakRef<ir::Mut>, What>,
}

enum What {
    Convert(ir::Ref<ir::Ssa>),
    Remove,
}

#[derive(Default)]
struct CollectMutOpInfo<'a> {
    mut_ops: HashMap<&'a ir::Ref<ir::Mut>, State>,
    reads_in_scope: HashSet<&'a ir::Ref<ir::Mut>>,
    declared_at_toplevel_of_switch: HashSet<&'a ir::Ref<ir::Mut>>,
    at_toplevel_of_switch: bool,
    about_to_enter_switch: bool,
}

#[derive(PartialEq)]
enum State {
    ReadOnly { frozen: bool },
    WriteOnly,
    Invalid,
}

impl<'a> Visitor<'a> for CollectMutOpInfo<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                // functions are analyzed separately, since they can read ssa refs
                // before they are lexically declared
                let mut inner = Self::default();
                mem::swap(&mut inner.mut_ops, &mut self.mut_ops);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.mut_ops, &mut self.mut_ops);
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel | ScopeTy::Nonlinear => {
                let mut inner = Self::default();
                mem::swap(&mut inner.mut_ops, &mut self.mut_ops);
                mem::swap(&mut inner.reads_in_scope, &mut self.reads_in_scope);
                inner.at_toplevel_of_switch = self.about_to_enter_switch;
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.mut_ops, &mut self.mut_ops);
                mem::swap(&mut inner.reads_in_scope, &mut self.reads_in_scope);
                self.about_to_enter_switch = false;
                r
            }
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr {
                target: _,
                expr: ir::Expr::ReadMutable { source },
            } => {
                let our_state = State::ReadOnly { frozen: false };
                self.mut_ops
                    .entry(source)
                    .and_modify(|state| {
                        if state != &our_state {
                            *state = State::Invalid;
                        }
                    })
                    .or_insert(our_state);
                self.reads_in_scope.insert(source);
            }
            ir::Stmt::WriteMutable { target, val: _ } => {
                let our_state = State::WriteOnly;
                self.mut_ops
                    .entry(target)
                    .and_modify(|state| {
                        if state != &our_state {
                            *state = State::Invalid;
                        }
                    })
                    .or_insert(our_state);
            }
            ir::Stmt::DeclareMutable { target, val: _ } => {
                if self.reads_in_scope.contains(&target) {
                    // read before declaration
                    self.mut_ops.insert(target, State::Invalid);
                }
                if self.at_toplevel_of_switch {
                    self.declared_at_toplevel_of_switch.insert(target);
                }
            }
            ir::Stmt::Switch { .. } => {
                self.about_to_enter_switch = true;
            }
            ir::Stmt::SwitchCase { .. } => {
                for ref_ in self.declared_at_toplevel_of_switch.drain() {
                    match self.mut_ops.entry(ref_).or_insert(State::WriteOnly) {
                        State::ReadOnly { frozen } => {
                            *frozen = true;
                        }
                        State::WriteOnly | State::Invalid => {
                            // future reads already disallowed
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Folder for Mut2Ssa {
    type Output = Option<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut collector = CollectMutOpInfo::default();
            collector.run_visitor(&block);
            self.mut_vars_to_replace = collector
                .mut_ops
                .into_iter()
                .flat_map(|(ref_, saw)| match saw {
                    State::ReadOnly { frozen: _ } => {
                        Some((ref_.weak(), What::Convert(ir::Ref::new(ref_.name_hint()))))
                    }
                    State::WriteOnly => Some((ref_.weak(), What::Remove)),
                    State::Invalid => None,
                })
                .collect();
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::ReadMutable { source },
            } => match self.mut_vars_to_replace.get(&source.weak()) {
                Some(What::Convert(ssa_ref)) => Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Read {
                        source: ssa_ref.clone(),
                    },
                }),
                Some(What::Remove) => unreachable!("removing mut read: {:?}", source),
                None => Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::ReadMutable { source },
                }),
            },
            ir::Stmt::DeclareMutable { target, val } => {
                match self.mut_vars_to_replace.get(&target.weak()) {
                    Some(What::Convert(ssa_ref)) => Some(ir::Stmt::Expr {
                        target: ssa_ref.clone(),
                        expr: ir::Expr::Read { source: val },
                    }),
                    Some(What::Remove) => None,
                    None => Some(ir::Stmt::DeclareMutable { target, val }),
                }
            }
            ir::Stmt::WriteMutable { target, val } => {
                match self.mut_vars_to_replace.get(&target.weak()) {
                    Some(What::Convert(_)) => unreachable!("converting mut write: {:?}", target),
                    Some(What::Remove) => None,
                    None => Some(ir::Stmt::WriteMutable { target, val }),
                }
            }
            _ => Some(stmt),
        }
    }
}
