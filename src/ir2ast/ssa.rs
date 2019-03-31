use std::collections::{HashMap, HashSet};
use std::mem;

use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::visit::{RunVisitor, Visitor};

pub struct Cache {
    no_side_effects_between_def_and_use: HashSet<ir::WeakRef<ir::SSA>>,
    expr_cache: HashMap<ir::Ref<ir::SSA>, ast::Expr>,
}

impl Cache {
    pub fn with_inlining_information(ir: &ir::Block) -> Self {
        let mut collector = CollectSingleUseInliningInfo::default();
        collector.run_visitor(&ir);
        Self {
            no_side_effects_between_def_and_use: collector.no_side_effects_between_def_and_use,
            expr_cache: Default::default(),
        }
    }

    pub fn can_be_freely_inlined(&self, ssa_ref: &ir::Ref<ir::SSA>) -> bool {
        self.no_side_effects_between_def_and_use
            .contains(&ssa_ref.weak())
    }

    pub fn cache(&mut self, ssa_ref: ir::Ref<ir::SSA>, expr: ast::Expr) {
        let old_value = self.expr_cache.insert(ssa_ref, expr);
        assert!(old_value.is_none(), "SSA var cached multiple times");
    }

    pub fn get_cached(&self, ssa_ref: &ir::Ref<ir::SSA>) -> Option<&ast::Expr> {
        self.expr_cache.get(ssa_ref)
    }
}

#[derive(Default)]
struct CollectSingleUseInliningInfo {
    no_side_effects_between_def_and_use: HashSet<ir::WeakRef<ir::SSA>>,
    active_single_use_refs: HashSet<ir::WeakRef<ir::SSA>>,
}

impl CollectSingleUseInliningInfo {
    fn use_ref(&mut self, ssa_ref: &ir::Ref<ir::SSA>) {
        if self.active_single_use_refs.remove(&ssa_ref.weak()) {
            self.no_side_effects_between_def_and_use
                .insert(ssa_ref.weak());
        }
    }

    fn side_effect(&mut self) {
        self.active_single_use_refs.clear();
    }
}

impl Visitor for CollectSingleUseInliningInfo {
    fn wrap_scope<R>(&mut self, enter: impl FnOnce(&mut Self) -> R) -> R {
        // it is not safe in general to inline single-use refs between scopes,
        // since they may have side-effects
        let prev_scope_active = mem::replace(&mut self.active_single_use_refs, Default::default());
        let r = enter(self);
        self.active_single_use_refs = prev_scope_active;
        r
    }

    fn visit(&mut self, stmt: &ir::Stmt) {
        match stmt {
            ir::Stmt::Expr { target, expr } => {
                match expr {
                    ir::Expr::Read { source } => self.use_ref(source),
                    ir::Expr::ReadMember { obj, prop } => {
                        self.use_ref(obj);
                        self.use_ref(prop);
                    }
                    ir::Expr::Array { elems } => {
                        elems
                            .iter()
                            .flatten()
                            .for_each(|(_, elem)| self.use_ref(elem));
                    }
                    ir::Expr::Object { props } => {
                        props.iter().for_each(|(_, obj, prop)| {
                            self.use_ref(obj);
                            self.use_ref(prop);
                        });
                    }
                    ir::Expr::Unary { op: _, val } => self.use_ref(val),
                    ir::Expr::Binary { op: _, left, right } => {
                        self.use_ref(left);
                        self.use_ref(right);
                    }
                    ir::Expr::Delete { obj, prop } => {
                        self.use_ref(obj);
                        self.use_ref(prop);
                    }
                    ir::Expr::Yield { kind: _, val } => self.use_ref(val),
                    ir::Expr::Await { val } => self.use_ref(val),
                    ir::Expr::Call {
                        kind: _,
                        func,
                        args,
                    } => {
                        self.use_ref(func);
                        args.iter().for_each(|(_, arg)| self.use_ref(arg));
                    }
                    ir::Expr::Bool { .. }
                    | ir::Expr::Number { .. }
                    | ir::Expr::String { .. }
                    | ir::Expr::Null
                    | ir::Expr::Undefined
                    | ir::Expr::This
                    | ir::Expr::ReadMutable { .. }
                    | ir::Expr::ReadGlobal { .. }
                    | ir::Expr::Function { .. }
                    | ir::Expr::CurrentFunction
                    | ir::Expr::Argument { .. }
                    | ir::Expr::RegExp { .. } => {}
                }

                match expr {
                    ir::Expr::ReadMutable { .. }
                    | ir::Expr::ReadGlobal { .. }
                    | ir::Expr::ReadMember { .. }
                    | ir::Expr::Delete { .. }
                    | ir::Expr::Yield { .. }
                    | ir::Expr::Await { .. }
                    | ir::Expr::Call { .. } => {
                        self.side_effect();
                    }
                    ir::Expr::Bool { .. }
                    | ir::Expr::Number { .. }
                    | ir::Expr::String { .. }
                    | ir::Expr::Null
                    | ir::Expr::Undefined
                    | ir::Expr::This
                    | ir::Expr::Read { .. }
                    | ir::Expr::Array { .. }
                    | ir::Expr::Object { .. }
                    | ir::Expr::RegExp { .. }
                    | ir::Expr::Unary { .. }
                    | ir::Expr::Binary { .. }
                    | ir::Expr::Function { .. }
                    | ir::Expr::CurrentFunction
                    | ir::Expr::Argument { .. } => {}
                }

                if let ir::Used::Once = target.used() {
                    self.active_single_use_refs.insert(target.weak());
                }
            }
            ir::Stmt::DeclareMutable { target: _, val } => {
                self.use_ref(val);
            }
            ir::Stmt::WriteMutable { target: _, val } => {
                self.use_ref(val);
                self.side_effect();
            }
            ir::Stmt::WriteGlobal { target: _, val } => {
                self.use_ref(val);
                self.side_effect();
            }
            ir::Stmt::WriteMember { obj, prop, val } => {
                self.use_ref(obj);
                self.use_ref(prop);
                self.use_ref(val);
                self.side_effect();
            }
            ir::Stmt::Return { val } | ir::Stmt::Throw { val } => {
                self.use_ref(val);
                self.side_effect();
            }
            ir::Stmt::Break | ir::Stmt::Continue => {
                self.side_effect();
            }
            ir::Stmt::Debugger => {
                self.side_effect();
            }
            ir::Stmt::Loop { body: _ } => {
                self.side_effect();
            }
            ir::Stmt::ForEach {
                kind: _,
                init,
                body: _,
            } => {
                self.use_ref(init);
                self.side_effect();
            }
            ir::Stmt::IfElse {
                cond,
                cons: _,
                alt: _,
            } => {
                self.use_ref(cond);
                self.side_effect();
            }
            ir::Stmt::Try {
                body: _,
                catch: _,
                finally: _,
            } => {
                self.side_effect();
            }
        }
    }
}
