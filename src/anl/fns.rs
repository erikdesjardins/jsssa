use std::collections::HashSet;
use std::mem;

use crate::ir;
use crate::ir::traverse::{RunVisitor, ScopeTy, Visitor};

pub fn without_this(ir: &ir::Block) -> HashSet<&ir::Ref<ir::Ssa>> {
    let mut collector = WithoutThis::default();
    collector.run_visitor(ir);
    collector.fns_without_this
}

#[derive(Debug, Default)]
pub struct WithoutThis<'a> {
    fns_without_this: HashSet<&'a ir::Ref<ir::Ssa>>,
    about_to_enter_arrow_fn: bool,
    about_to_enter_fn: Option<&'a ir::Ref<ir::Ssa>>,
    current_fn: Option<&'a ir::Ref<ir::Ssa>>,
    used_this: bool,
}

impl<'a> Visitor<'a> for WithoutThis<'a> {
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
                mem::swap(&mut inner.fns_without_this, &mut self.fns_without_this);
                inner.current_fn = self.about_to_enter_fn.take();
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.fns_without_this, &mut self.fns_without_this);
                // if fn hasn't been invalidated, it's good
                self.fns_without_this.extend(inner.current_fn);
                // propagate `this` from arrow functions
                if is_arrow_fn && inner.used_this {
                    self.current_fn = None;
                    self.used_this = true;
                }
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel | ScopeTy::Nonlinear => enter(self, block),
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::Function { kind, body: _ },
            } => {
                match kind {
                    ir::FnKind::Arrow { .. } => {
                        self.about_to_enter_arrow_fn = true;
                    }
                    ir::FnKind::Func { .. } => {}
                }
                self.about_to_enter_fn = Some(target);
            }
            ir::Stmt::Expr {
                target: _,
                expr: ir::Expr::This,
            } => {
                self.current_fn = None;
                self.used_this = true;
            }
            _ => {}
        }
    }
}
