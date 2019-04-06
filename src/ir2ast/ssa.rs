use std::collections::{HashMap, HashSet};
use std::iter;
use std::mem;

use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::traverse::{RunVisitor, ScopeTy, Visitor};

pub struct Cache {
    can_inline_at_use: HashSet<ir::WeakRef<ir::Ssa>>,
    expr_cache: HashMap<ir::WeakRef<ir::Ssa>, ast::Expr>,
    to_do_at_declaration: HashMap<ir::WeakRef<ir::Ssa>, ToDo>,
}

impl Cache {
    #[inline(never)] // for better profiling
    pub fn prepare_for_inlining(ir: &ir::Block) -> Self {
        let mut collector = CollectSingleUseInliningInfo::default();
        collector.run_visitor(&ir);
        Self {
            can_inline_at_use: collector
                .can_inline_at_use
                .into_iter()
                .map(ir::Ref::weak)
                .collect(),
            expr_cache: Default::default(),
            to_do_at_declaration: Default::default(),
        }
    }

    pub fn empty() -> Self {
        Self {
            can_inline_at_use: Default::default(),
            expr_cache: Default::default(),
            to_do_at_declaration: Default::default(),
        }
    }
}

pub enum ToDo {
    EmitForSideEffects,
    AddToCache,
    DropAlreadyCached,
    DeclareVar,
}

impl Cache {
    pub fn can_be_inlined_forwards(&self, ssa_ref: &ir::Ref<ir::Ssa>) -> bool {
        self.can_inline_at_use.contains(&ssa_ref.weak())
    }

    pub fn cache(&mut self, ssa_ref: &ir::Ref<ir::Ssa>, expr: ast::Expr) {
        let old_value = self.expr_cache.insert(ssa_ref.weak(), expr);
        assert!(old_value.is_none(), "cached multiple times: {:?}", ssa_ref);
    }

    pub fn get_cached(&self, ssa_ref: &ir::Ref<ir::Ssa>) -> Option<&ast::Expr> {
        self.expr_cache.get(&ssa_ref.weak())
    }

    pub fn do_at_declaration(&mut self, ssa_ref: &ir::Ref<ir::Ssa>, to_do: ToDo) {
        let old_value = self.to_do_at_declaration.insert(ssa_ref.weak(), to_do);
        assert!(old_value.is_none(), "multiple todos for ref: {:?}", ssa_ref);
    }

    pub fn what_to_do(&self, ssa_ref: &ir::Ref<ir::Ssa>) -> Option<&ToDo> {
        self.to_do_at_declaration.get(&ssa_ref.weak())
    }
}

#[derive(Default)]
struct CollectSingleUseInliningInfo<'a> {
    can_inline_at_use: HashSet<&'a ir::Ref<ir::Ssa>>,
    pure_refs: HashSet<&'a ir::Ref<ir::Ssa>>,
    read_refs: HashSet<&'a ir::Ref<ir::Ssa>>,
    write_refs: HashSet<&'a ir::Ref<ir::Ssa>>,
    largest_effect: Effect,
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

impl<'a> CollectSingleUseInliningInfo<'a> {
    fn declare_ref(&mut self, ref_: &'a ir::Ref<ir::Ssa>, eff: Effect) {
        let added_to_set = match eff {
            Effect::Pure => self.pure_refs.insert(ref_),
            Effect::Read => self.read_refs.insert(ref_),
            Effect::Write => self.write_refs.insert(ref_),
        };
        assert!(added_to_set, "refs can only be declared once");
    }

    fn use_ref(&mut self, own_eff: Effect, ref_: &'a ir::Ref<ir::Ssa>) -> Effect {
        self.use_refs(own_eff, iter::once(ref_))
    }

    fn use_refs_<'b>(
        &mut self,
        own_eff: Effect,
        refs: impl IntoIterator<Item = &'b &'a ir::Ref<ir::Ssa>>,
    ) -> Effect
    where
        'a: 'b,
    {
        self.use_refs(own_eff, refs.into_iter().map(|r| *r))
    }

    fn use_refs(
        &mut self,
        own_eff: Effect,
        refs: impl IntoIterator<Item = &'a ir::Ref<ir::Ssa>>,
    ) -> Effect {
        refs.into_iter()
            .filter_map(|ref_| {
                if self.pure_refs.remove(&ref_) {
                    self.can_inline_at_use.insert(ref_);
                    Some(Effect::Pure)
                } else if self.read_refs.remove(&ref_) {
                    self.can_inline_at_use.insert(ref_);
                    Some(Effect::Read)
                } else if self.write_refs.remove(&ref_) {
                    self.can_inline_at_use.insert(ref_);
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

impl<'a> Visitor<'a> for CollectSingleUseInliningInfo<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                // each function is analyzed separately, but keep the map of results
                let mut inner = Self::default();
                mem::swap(&mut inner.can_inline_at_use, &mut self.can_inline_at_use);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.can_inline_at_use, &mut self.can_inline_at_use);
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel => {
                // write refs cannot be inlined into normal scopes (because they may not execute)
                let mut inner = Self::default();
                mem::swap(&mut inner.can_inline_at_use, &mut self.can_inline_at_use);
                mem::swap(&mut inner.pure_refs, &mut self.pure_refs);
                mem::swap(&mut inner.read_refs, &mut self.read_refs);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.can_inline_at_use, &mut self.can_inline_at_use);
                mem::swap(&mut inner.pure_refs, &mut self.pure_refs);
                mem::swap(&mut inner.read_refs, &mut self.read_refs);
                // apply side effect from inner scope
                self.side_effect(inner.largest_effect);
                r
            }
            ScopeTy::Nonlinear => {
                // read and write refs cannot be inlined into nonlinear scopes
                let mut inner = Self::default();
                mem::swap(&mut inner.can_inline_at_use, &mut self.can_inline_at_use);
                mem::swap(&mut inner.pure_refs, &mut self.pure_refs);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.can_inline_at_use, &mut self.can_inline_at_use);
                mem::swap(&mut inner.pure_refs, &mut self.pure_refs);
                // apply side effect from inner scope
                self.side_effect(inner.largest_effect);
                r
            }
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
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
                    ir::Expr::Function { .. } => Effect::Pure,
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
            ir::Stmt::Switch { discr, body: _ } => {
                let eff = self.use_ref(Effect::Pure, discr);
                self.side_effect(eff);
            }
            ir::Stmt::SwitchCase { val } => {
                let eff = self.use_refs(Effect::Read, val);
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
