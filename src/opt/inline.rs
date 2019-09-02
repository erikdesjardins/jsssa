use std::collections::{HashMap, HashSet};
use std::iter;
use std::mem;

use crate::collections::ZeroOneMany::{self, Many, One, Zero};
use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Inline functions called a single time.
///
/// Does not profit from multiple passes.
/// May profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for _everything_; in particular, read forwarding.
#[derive(Debug, Default)]
pub struct Inline {
    fns_to_inline: HashSet<ir::WeakRef<ir::Ssa>>,
    fn_bodies: HashMap<ir::WeakRef<ir::Ssa>, ir::Block>,
}

impl Drop for Inline {
    fn drop(&mut self) {
        assert!(
            self.fn_bodies.is_empty(),
            "should have inlined fns: {:?}",
            self.fn_bodies
        );
    }
}

#[derive(Debug, Default)]
struct CollectFnCallInfo<'a> {
    fns_to_inline: HashSet<&'a ir::Ref<ir::Ssa>>,
    fn_def_is_good: HashSet<&'a ir::Ref<ir::Ssa>>,
    about_to_enter_arrow_fn: bool,
    about_to_enter_fn: Option<&'a ir::Ref<ir::Ssa>>,
    current_fn: Option<&'a ir::Ref<ir::Ssa>>,
    depth_in_current_fn: u32,
    used_this: bool,
}

impl<'a> Visitor<'a> for CollectFnCallInfo<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                let is_arrow_fn = mem::replace(&mut self.about_to_enter_arrow_fn, false);
                let mut inner = Self::default();
                mem::swap(&mut inner.fns_to_inline, &mut self.fns_to_inline);
                mem::swap(&mut inner.fn_def_is_good, &mut self.fn_def_is_good);
                inner.current_fn = self.about_to_enter_fn.take();
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.fns_to_inline, &mut self.fns_to_inline);
                mem::swap(&mut inner.fn_def_is_good, &mut self.fn_def_is_good);
                // if fn hasn't been invalidated, it's good
                self.fn_def_is_good.extend(inner.current_fn);
                // propagate `this` from arrow functions
                if is_arrow_fn && inner.used_this {
                    self.current_fn = None;
                    self.used_this = true;
                }
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel | ScopeTy::Nonlinear => {
                self.depth_in_current_fn += 1;
                let r = enter(self, block);
                self.depth_in_current_fn -= 1;
                r
            }
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr { target, expr } => match expr {
                ir::Expr::Function {
                    kind:
                        ir::FnKind::Func {
                            is_async: false,
                            is_generator: false,
                        },
                    body: _,
                } if target.used().is_once() => {
                    // fn is simple, used once: def is good if its body is good
                    self.about_to_enter_fn = Some(target);
                }
                ir::Expr::Function { kind, body: _ } => {
                    self.about_to_enter_arrow_fn = true;
                    match kind {
                        ir::FnKind::Arrow { is_async: false } if target.used().is_once() => {
                            // fn is simple, used once: def is good if its body is good
                            self.about_to_enter_fn = Some(target);
                        }
                        _ => {}
                    }
                }
                ir::Expr::Call {
                    kind: ir::CallKind::Call,
                    base,
                    prop: None,
                    args,
                } if self.fn_def_is_good.contains(base) => {
                    if args.iter().all(|(kind, _)| match kind {
                        ir::EleKind::Single => true,
                        ir::EleKind::Spread => false,
                    }) {
                        // fn def is good, and call is simple: mark for inlining
                        self.fns_to_inline.insert(base);
                    }
                }
                ir::Expr::This => {
                    // fn body uses `this`: invalidate
                    self.current_fn = None;
                    self.used_this = true;
                }
                ir::Expr::CurrentFunction => {
                    // fn is possibly recursive: invalidate
                    self.current_fn = None;
                }
                _ => {}
            },
            ir::Stmt::Return { .. } if self.depth_in_current_fn > 0 => {
                // return statement not at top level: invalidate
                self.current_fn = None;
            }
            _ => {}
        }
    }
}

impl Folder for Inline {
    type Output = ZeroOneMany<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut collector = CollectFnCallInfo::default();
            collector.run_visitor(&block);
            self.fns_to_inline = collector
                .fns_to_inline
                .into_iter()
                .map(ir::Ref::weak)
                .collect();
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::Function { kind, body },
            } => {
                if self.fns_to_inline.contains(&target.weak()) {
                    self.fn_bodies.insert(target.weak(), body);
                    Zero
                } else {
                    One(ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::Function { kind, body },
                    })
                }
            }
            ir::Stmt::Expr {
                target,
                expr:
                    ir::Expr::Call {
                        kind: ir::CallKind::Call,
                        base,
                        prop: None,
                        args,
                    },
            } => {
                if let Some(ir::Block(body)) = self.fn_bodies.remove(&base.weak()) {
                    let undef_ref = ir::Ref::new("_mis");
                    let undef = ir::Stmt::Expr {
                        target: undef_ref.clone(),
                        expr: ir::Expr::Undefined,
                    };
                    let mut return_ref = None;
                    let body = body
                        .into_iter()
                        .flat_map(|stmt| match stmt {
                            ir::Stmt::Expr {
                                target: _,
                                expr: ir::Expr::CurrentFunction,
                            } => unreachable!("curfn should have been removed"),
                            ir::Stmt::Expr {
                                target,
                                expr: ir::Expr::Argument { index },
                            } => Some(ir::Stmt::Expr {
                                target,
                                expr: ir::Expr::Read {
                                    source: args
                                        .get(index)
                                        .map(|(_, arg)| arg.clone())
                                        .unwrap_or_else(|| undef_ref.clone()),
                                },
                            }),
                            ir::Stmt::Return { val } => {
                                assert!(return_ref.is_none(), "should only have 1 return");
                                return_ref = Some(val);
                                None
                            }
                            _ => Some(stmt),
                        })
                        .collect::<Vec<_>>();
                    let ret = ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::Read {
                            source: return_ref.unwrap_or(undef_ref),
                        },
                    };
                    Many(
                        iter::once(undef)
                            .chain(body)
                            .chain(iter::once(ret))
                            .collect(),
                    )
                } else {
                    One(ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::Call {
                            kind: ir::CallKind::Call,
                            base,
                            prop: None,
                            args,
                        },
                    })
                }
            }
            _ => One(stmt),
        }
    }
}
