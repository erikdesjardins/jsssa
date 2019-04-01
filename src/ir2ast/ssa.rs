use std::collections::{HashMap, HashSet};
use std::iter;
use std::mem;

use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::visit::{RunVisitor, Visitor};

pub struct Cache {
    can_inline_at_use: HashSet<ir::WeakRef<ir::Ssa>>,
    expr_cache: HashMap<ir::Ref<ir::Ssa>, ast::Expr>,
}

impl Cache {
    pub fn with_inlining_information(ir: &ir::Block) -> Self {
        let mut collector = CollectSingleUseInliningInfo::default();
        collector.run_visitor(&ir);
        Self {
            can_inline_at_use: collector.can_inline_at_use,
            expr_cache: Default::default(),
        }
    }

    pub fn can_be_freely_inlined(&self, ssa_ref: &ir::Ref<ir::Ssa>) -> bool {
        self.can_inline_at_use.contains(&ssa_ref.weak())
    }

    pub fn cache(&mut self, ssa_ref: ir::Ref<ir::Ssa>, expr: ast::Expr) {
        let old_value = self.expr_cache.insert(ssa_ref, expr);
        assert!(old_value.is_none(), "ssa var cached multiple times");
    }

    pub fn get_cached(&self, ssa_ref: &ir::Ref<ir::Ssa>) -> Option<&ast::Expr> {
        self.expr_cache.get(ssa_ref)
    }
}

#[derive(Default)]
struct CollectSingleUseInliningInfo {
    can_inline_at_use: HashSet<ir::WeakRef<ir::Ssa>>,
    pure_refs: HashSet<ir::WeakRef<ir::Ssa>>,
    read_refs: HashSet<ir::WeakRef<ir::Ssa>>,
    write_refs: HashSet<ir::WeakRef<ir::Ssa>>,
    largest_effect: Effect,
    about_to_enter_fn: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Effect {
    Pure,
    Read,
    Write,
}

impl Default for Effect {
    fn default() -> Self {
        Effect::Pure
    }
}

impl CollectSingleUseInliningInfo {
    fn declare_ref(&mut self, ref_: &ir::Ref<ir::Ssa>, eff: Effect) {
        let added_to_set = match eff {
            Effect::Pure => self.pure_refs.insert(ref_.weak()),
            Effect::Read => self.read_refs.insert(ref_.weak()),
            Effect::Write => self.write_refs.insert(ref_.weak()),
        };
        assert!(added_to_set, "refs can only be declared once");
    }

    fn use_ref(&mut self, own_eff: Effect, ref_: &ir::Ref<ir::Ssa>) -> Effect {
        self.use_refs(own_eff, iter::once(ref_))
    }

    fn use_refs_<'a, 'b: 'a>(
        &mut self,
        own_eff: Effect,
        refs: impl IntoIterator<Item = &'a &'b ir::Ref<ir::Ssa>>,
    ) -> Effect {
        self.use_refs(own_eff, refs.into_iter().map(|r| *r))
    }

    fn use_refs<'a>(
        &mut self,
        own_eff: Effect,
        refs: impl IntoIterator<Item = &'a ir::Ref<ir::Ssa>>,
    ) -> Effect {
        refs.into_iter()
            .filter_map(|ref_| {
                if self.pure_refs.remove(&ref_.weak()) {
                    self.can_inline_at_use.insert(ref_.weak());
                    Some(Effect::Pure)
                } else if self.read_refs.remove(&ref_.weak()) {
                    self.can_inline_at_use.insert(ref_.weak());
                    Some(Effect::Read)
                } else if self.write_refs.remove(&ref_.weak()) {
                    self.can_inline_at_use.insert(ref_.weak());
                    Some(Effect::Write)
                } else {
                    None
                }
            })
            .chain(iter::once(own_eff))
            .max()
            .unwrap_or(Effect::Write)
    }

    fn side_effect(&mut self, eff: Effect) {
        match &eff {
            Effect::Pure => {}
            Effect::Read => {
                self.write_refs.clear();
            }
            Effect::Write => {
                self.read_refs.clear();
                self.write_refs.clear();
            }
        }
        if eff > self.largest_effect {
            self.largest_effect = eff;
        }
    }
}

impl Visitor for CollectSingleUseInliningInfo {
    fn wrap_scope<R>(&mut self, enter: impl FnOnce(&mut Self) -> R) -> R {
        // pure refs can be inlined in any scope, but not between functions
        let pure_refs = if self.about_to_enter_fn {
            self.about_to_enter_fn = false;
            Some(mem::replace(&mut self.pure_refs, Default::default()))
        } else {
            None
        };
        // read and write refs cannot be inlined into an inner scope, but may be inlined across it
        let read_refs = mem::replace(&mut self.read_refs, Default::default());
        let write_refs = mem::replace(&mut self.write_refs, Default::default());
        let largest_effect = mem::replace(&mut self.largest_effect, Default::default());

        let r = enter(self);

        if let Some(pure_refs) = pure_refs {
            self.pure_refs = pure_refs;
        }
        self.read_refs = read_refs;
        self.write_refs = write_refs;
        let inner_largest_effect = mem::replace(&mut self.largest_effect, largest_effect);

        // apply side effect from inner scope, possibly clearing read/write refs and raising our effect level
        self.side_effect(inner_largest_effect);

        r
    }

    fn visit(&mut self, stmt: &ir::Stmt) {
        match stmt {
            ir::Stmt::Expr { target, expr } => {
                let eff = match expr {
                    ir::Expr::Bool { .. }
                    | ir::Expr::Number { .. }
                    | ir::Expr::String { .. }
                    | ir::Expr::Null
                    | ir::Expr::Undefined => Effect::Pure,
                    ir::Expr::This => Effect::Pure,
                    ir::Expr::Read { source } => self.use_ref(Effect::Read, source),
                    ir::Expr::ReadMutable { .. } => Effect::Read,
                    ir::Expr::ReadGlobal { .. } => Effect::Read,
                    ir::Expr::ReadMember { obj, prop } => {
                        self.use_refs_(Effect::Read, &[obj, prop])
                    }
                    ir::Expr::Array { elems } => {
                        self.use_refs(Effect::Pure, elems.iter().flatten().map(|(_, elem)| elem))
                    }
                    ir::Expr::Object { props } => self.use_refs(
                        Effect::Pure,
                        props
                            .iter()
                            .flat_map(|(_, obj, prop)| iter::once(obj).chain(iter::once(prop))),
                    ),
                    ir::Expr::RegExp { .. } => Effect::Read,
                    ir::Expr::Unary { op: _, val } => self.use_ref(Effect::Pure, val),
                    ir::Expr::Binary { op: _, left, right } => {
                        self.use_refs_(Effect::Pure, &[left, right])
                    }
                    ir::Expr::Delete { obj, prop } => self.use_refs_(Effect::Write, &[obj, prop]),
                    ir::Expr::Yield { kind: _, val } => self.use_ref(Effect::Write, val),
                    ir::Expr::Await { val } => self.use_ref(Effect::Write, val),
                    ir::Expr::Call {
                        kind: _,
                        func,
                        args,
                    } => self.use_refs(
                        Effect::Write,
                        iter::once(func).chain(args.iter().map(|(_, arg)| arg)),
                    ),
                    ir::Expr::Function { .. } => {
                        self.about_to_enter_fn = true;
                        Effect::Pure
                    }
                    ir::Expr::CurrentFunction | ir::Expr::Argument { .. } => Effect::Read,
                };

                self.side_effect(eff.clone());

                if let ir::Used::Once = target.used() {
                    self.declare_ref(&target, eff);
                }
            }
            ir::Stmt::DeclareMutable { target: _, val } => {
                let eff = self.use_ref(Effect::Pure, val);
                self.side_effect(eff);
            }
            ir::Stmt::WriteMutable { target: _, val } => {
                let eff = self.use_ref(Effect::Write, val);
                self.side_effect(eff);
            }
            ir::Stmt::WriteGlobal { target: _, val } => {
                let eff = self.use_ref(Effect::Write, val);
                self.side_effect(eff);
            }
            ir::Stmt::WriteMember { obj, prop, val } => {
                let eff = self.use_refs_(Effect::Write, &[obj, prop, val]);
                self.side_effect(eff);
            }
            ir::Stmt::Return { val } | ir::Stmt::Throw { val } => {
                let eff = self.use_ref(Effect::Read, val);
                self.side_effect(eff);
            }
            ir::Stmt::Break { .. } | ir::Stmt::Continue { .. } => {
                self.side_effect(Effect::Read);
            }
            ir::Stmt::Debugger => {
                self.side_effect(Effect::Write);
            }
            ir::Stmt::Label { label: _, body: _ } => {
                self.side_effect(Effect::Pure);
            }
            ir::Stmt::Loop { body: _ } => {
                self.side_effect(Effect::Pure);
            }
            ir::Stmt::ForEach {
                kind: _,
                init,
                body: _,
            } => {
                let eff = self.use_ref(Effect::Pure, init);
                self.side_effect(eff);
            }
            ir::Stmt::IfElse {
                cond,
                cons: _,
                alt: _,
            } => {
                let eff = self.use_ref(Effect::Pure, cond);
                self.side_effect(eff);
            }
            ir::Stmt::Try {
                body: _,
                catch: _,
                finally: _,
            } => {
                self.side_effect(Effect::Pure);
            }
        }
    }
}
