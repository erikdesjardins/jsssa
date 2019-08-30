use std::collections::HashSet;

use crate::ir;
use crate::ir::traverse::{RunVisitor, ScopeTy, Visitor};

pub fn used_in_only_one_fn_scope(ir: &ir::Block) -> HashSet<&ir::Ref<ir::Ssa>> {
    let mut collector = UsedInOnlyOneFnScope::default();
    collector.run_visitor(ir);
    collector.declared_refs
}

#[derive(Default)]
struct UsedInOnlyOneFnScope<'a> {
    declared_refs: HashSet<&'a ir::Ref<ir::Ssa>>,
    used_refs: HashSet<&'a ir::Ref<ir::Ssa>>,
}

impl<'a> Visitor<'a> for UsedInOnlyOneFnScope<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Toplevel | ScopeTy::Function => {
                let mut inner = Self::default();
                let r = enter(&mut inner, block);
                for used_in_inner_scope in &inner.used_refs {
                    self.declared_refs.remove(used_in_inner_scope);
                }
                self.declared_refs.extend(inner.declared_refs);
                self.used_refs.extend(inner.used_refs);
                r
            }
            ScopeTy::Normal | ScopeTy::Nonlinear => enter(self, block),
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr { target, expr: _ } => {
                // avoid time travel
                if !self.used_refs.contains(target) {
                    self.declared_refs.insert(target);
                }
            }
            _ => {}
        }
    }

    fn visit_ref_use(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
        self.used_refs.insert(ref_);
    }
}
