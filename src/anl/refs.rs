use std::collections::HashMap;

use crate::ir;
use crate::ir::traverse::{RunVisitor, ScopeTy, Visitor};

pub fn used_in_only_one_fn_scope<'a, K>(ir: &'a ir::Block) -> impl Iterator<Item = &'a ir::Ref<K>>
where
    K: ir::RefType + 'a,
    UsedInOnlyOneFnScope<'a, K>: Visitor<'a>,
{
    let mut collector = UsedInOnlyOneFnScope::default();
    collector.run_visitor(ir);
    collector
        .refs
        .into_iter()
        .filter_map(|(ref_, state)| match state {
            State::Valid(_) => Some(ref_),
            State::Invalid => None,
        })
}

type LevelNumber = u64;

#[derive(Debug)]
pub struct UsedInOnlyOneFnScope<'a, K: ir::RefType> {
    refs: HashMap<&'a ir::Ref<K>, State>,
    cur_level: LevelNumber,
}

#[derive(Debug)]
enum State {
    Valid(LevelNumber),
    Invalid,
}

impl<'a, K: ir::RefType> Default for UsedInOnlyOneFnScope<'a, K> {
    fn default() -> Self {
        Self {
            refs: Default::default(),
            cur_level: Default::default(),
        }
    }
}

impl<'a, K: ir::RefType> UsedInOnlyOneFnScope<'a, K> {
    fn visit_ref(&mut self, ref_: &'a ir::Ref<K>) {
        let state = self
            .refs
            .entry(ref_)
            .or_insert(State::Valid(self.cur_level));
        match state {
            State::Valid(level) if *level != self.cur_level => {
                *state = State::Invalid;
            }
            State::Valid(_) | State::Invalid => {
                // only used in current scope or already invalid
            }
        }
    }
}

impl<'a> Visitor<'a> for UsedInOnlyOneFnScope<'a, ir::Ssa> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                self.cur_level += 1;
                let r = enter(self, block);
                self.cur_level -= 1;
                r
            }
            ScopeTy::Toplevel | ScopeTy::Normal | ScopeTy::Nonlinear => enter(self, block),
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr { target, expr: _ } => {
                self.visit_ref(target);
            }
            _ => {}
        }
    }

    fn visit_ref_use(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
        self.visit_ref(ref_);
    }
}

impl<'a> Visitor<'a> for UsedInOnlyOneFnScope<'a, ir::Mut> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                self.cur_level += 1;
                let r = enter(self, block);
                self.cur_level -= 1;
                r
            }
            ScopeTy::Toplevel | ScopeTy::Normal | ScopeTy::Nonlinear => enter(self, block),
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr {
                target: _,
                expr: ir::Expr::ReadMutable { source: ref_ },
            }
            | ir::Stmt::DeclareMutable {
                target: ref_,
                val: _,
            }
            | ir::Stmt::WriteMutable {
                target: ref_,
                val: _,
            } => {
                self.visit_ref(ref_);
            }
            _ => {}
        }
    }
}
