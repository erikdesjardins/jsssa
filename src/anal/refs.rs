use std::collections::HashMap;

use crate::ir;
use crate::ir::traverse::{RunVisitor, ScopeTy, Visitor};

pub fn used_in_only_one_fn_scope(ir: &ir::Block) -> impl Iterator<Item = &ir::Ref<ir::Ssa>> {
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

#[derive(Default)]
struct UsedInOnlyOneFnScope<'a> {
    refs: HashMap<&'a ir::Ref<ir::Ssa>, State>,
    cur_level: LevelNumber,
}

enum State {
    Valid(LevelNumber),
    Invalid,
}

impl<'a> UsedInOnlyOneFnScope<'a> {
    fn visit_ref(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
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

impl<'a> Visitor<'a> for UsedInOnlyOneFnScope<'a> {
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
