use std::collections::{HashMap, HashSet};
use std::mem;

use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Remove or forward redundant loads and stores of mutable vars.
///
/// May profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for mut-to-ssa downleveling.
/// May create opportunities for read forwarding.
///
/// Performs the following transformations:
///
/// Read-to-read forwarding:
///   something :- <null>
///   /* stuff */
///   x = something
///   y = something
/// ->
///   something :- <null>
///   /* stuff */
///   x = something
///   y = x
///
/// Write-to-read forwarding:
///   something :- <null>
///   /* stuff */
///   x = 1 + 1
///   something <- x
///   y = something
/// ->
///   something :- <null>
///   /* stuff */
///   x = 1 + 1
///   something <- x
///   y = x
///
/// Dead write elimination:
///   something :- <null>
///   /* stuff */
///   something <- x
///   something <- y
/// ->
///   something :- <null>
///   /* stuff */
///   something <- y
///
#[derive(Default)]
pub struct LoadStore {
    mut_ops_to_replace: HashMap<StmtIndex, What>,
    cur_index: StmtIndex,
}

type StmtIndex = u64;

enum What {
    ReadSsa(ir::Ref<ir::Ssa>),
    Remove,
    BecomeDecl,
}

#[derive(Default)]
struct CollectLoadStoreInfo {
    mut_ops_to_replace: HashMap<StmtIndex, What>,
    cur_index: StmtIndex,
    last_op_for_reads: HashMap<ir::WeakRef<ir::Mut>, (StmtIndex, MutOp)>,
    last_op_for_writes: HashMap<ir::WeakRef<ir::Mut>, (StmtIndex, MutOp)>,
    invalid_for_parent_scope: Invalid,
}

#[derive(Clone)]
enum MutOp {
    Declare(ir::Ref<ir::Ssa>),
    Write(ir::Ref<ir::Ssa>),
    Read(ir::Ref<ir::Ssa>),
}

enum Invalid {
    Everything,
    Refs(HashSet<ir::WeakRef<ir::Mut>>),
}

impl Default for Invalid {
    fn default() -> Self {
        Invalid::Refs(Default::default())
    }
}

impl Invalid {
    fn insert_ref(&mut self, ref_: ir::WeakRef<ir::Mut>) {
        match self {
            Invalid::Everything => {}
            Invalid::Refs(our_refs) => {
                our_refs.insert(ref_);
            }
        }
    }
}

impl CollectLoadStoreInfo {
    fn invalidate_from_child(&mut self, invalid: Invalid) {
        match invalid {
            Invalid::Everything => self.invalidate_everything(),
            Invalid::Refs(refs) => {
                for ref_ in refs {
                    self.last_op_for_reads.remove(&ref_);
                    self.last_op_for_writes.remove(&ref_);
                    self.invalid_for_parent_scope.insert_ref(ref_);
                }
            }
        }
    }

    fn invalidate_everything(&mut self) {
        self.last_op_for_reads.clear();
        self.last_op_for_writes.clear();
        self.invalid_for_parent_scope = Invalid::Everything;
    }

    fn declare_mut(&mut self, target: &ir::Ref<ir::Mut>, val: &ir::Ref<ir::Ssa>) {
        let op = (self.cur_index, MutOp::Declare(val.clone()));
        self.last_op_for_reads.insert(target.weak(), op.clone());
        self.last_op_for_writes.insert(target.weak(), op);
        self.invalid_for_parent_scope.insert_ref(target.weak());
    }

    fn write_mut(&mut self, target: &ir::Ref<ir::Mut>, val: &ir::Ref<ir::Ssa>) {
        let op = match self.last_op_for_writes.get(&target.weak()) {
            // write -> write (decl)
            Some((declare_index, MutOp::Declare(_))) => {
                self.mut_ops_to_replace.insert(*declare_index, What::Remove);
                self.mut_ops_to_replace
                    .insert(self.cur_index, What::BecomeDecl);
                (self.cur_index, MutOp::Declare(val.clone()))
            }
            // write -> write
            Some((write_index, MutOp::Write(_))) => {
                self.mut_ops_to_replace.insert(*write_index, What::Remove);
                (self.cur_index, MutOp::Write(val.clone()))
            }
            Some((_, MutOp::Read(_))) | None => (self.cur_index, MutOp::Write(val.clone())),
        };
        self.last_op_for_reads.insert(target.weak(), op.clone());
        self.last_op_for_writes.insert(target.weak(), op);
        self.invalid_for_parent_scope.insert_ref(target.weak());
    }

    fn read_mut(&mut self, target: &ir::Ref<ir::Ssa>, source: &ir::Ref<ir::Mut>) {
        let op = match self.last_op_for_reads.get(&source.weak()) {
            // write -> read, read -> read
            Some((_, MutOp::Declare(val)))
            | Some((_, MutOp::Write(val)))
            | Some((_, MutOp::Read(val))) => {
                self.mut_ops_to_replace
                    .insert(self.cur_index, What::ReadSsa(val.clone()));
                (self.cur_index, MutOp::Read(val.clone()))
            }
            None => (self.cur_index, MutOp::Read(target.clone())),
        };
        self.last_op_for_reads.insert(source.weak(), op.clone());
        self.last_op_for_writes.insert(source.weak(), op);
    }
}

impl Visitor for CollectLoadStoreInfo {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &ir::Block,
        enter: impl FnOnce(&mut Self, &ir::Block) -> R,
    ) -> R {
        match ty {
            ScopeTy::Function => {
                // function scopes are analyzed separately
                let mut inner = Self::default();
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel => {
                // r->r and w->r can go into scopes, but not w->w (since it might not execute)
                let mut inner = Self::default();
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.last_op_for_reads, &mut self.last_op_for_reads);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.last_op_for_reads, &mut self.last_op_for_reads);
                // invalidate any vars written in the inner scope
                self.invalidate_from_child(inner.invalid_for_parent_scope);
                r
            }
            ScopeTy::Nonlinear => {
                // forwarding can happen across nonlinear scopes, but not into them
                let mut inner = Self::default();
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                // invalidate any vars written in the inner scope
                self.invalidate_from_child(inner.invalid_for_parent_scope);
                r
            }
        }
    }

    fn visit(&mut self, stmt: &ir::Stmt) {
        self.cur_index += 1;
        match stmt {
            ir::Stmt::Expr { target, expr } => match expr {
                ir::Expr::ReadMutable { source } => self.read_mut(target, source),
                ir::Expr::Yield { .. } | ir::Expr::Await { .. } | ir::Expr::Call { .. } => {
                    self.invalidate_everything()
                }
                ir::Expr::Bool { .. }
                | ir::Expr::Number { .. }
                | ir::Expr::String { .. }
                | ir::Expr::Null
                | ir::Expr::Undefined
                | ir::Expr::This
                | ir::Expr::Read { .. }
                | ir::Expr::ReadGlobal { .. }
                | ir::Expr::ReadMember { .. }
                | ir::Expr::Array { .. }
                | ir::Expr::Object { .. }
                | ir::Expr::RegExp { .. }
                | ir::Expr::Unary { .. }
                | ir::Expr::Binary { .. }
                | ir::Expr::Delete { .. }
                | ir::Expr::Function { .. }
                | ir::Expr::CurrentFunction { .. }
                | ir::Expr::Argument { .. } => {}
            },
            ir::Stmt::DeclareMutable { target, val } => self.declare_mut(target, val),
            ir::Stmt::WriteMutable { target, val } => self.write_mut(target, val),
            ir::Stmt::Debugger { .. } => self.invalidate_everything(),
            ir::Stmt::WriteGlobal { .. }
            | ir::Stmt::WriteMember { .. }
            | ir::Stmt::Return { .. }
            | ir::Stmt::Throw { .. }
            | ir::Stmt::Break { .. }
            | ir::Stmt::Continue { .. }
            | ir::Stmt::Label { .. }
            | ir::Stmt::Loop { .. }
            | ir::Stmt::ForEach { .. }
            | ir::Stmt::IfElse { .. }
            | ir::Stmt::Try { .. } => {}
        }
    }
}

impl Folder for LoadStore {
    type Output = Option<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut collector = CollectLoadStoreInfo::default();
            collector.run_visitor(&block);
            self.mut_ops_to_replace = collector.mut_ops_to_replace;
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        self.cur_index += 1;
        match stmt {
            ir::Stmt::Expr {
                ref target,
                expr: ir::Expr::ReadMutable { .. },
            } => match self.mut_ops_to_replace.get(&self.cur_index) {
                Some(What::ReadSsa(ssa_ref)) => Some(ir::Stmt::Expr {
                    target: target.clone(),
                    expr: ir::Expr::Read {
                        source: ssa_ref.clone(),
                    },
                }),
                Some(What::Remove) | Some(What::BecomeDecl) => {
                    unreachable!("cannot remove convert mut read to decl")
                }
                None => Some(stmt),
            },
            ir::Stmt::DeclareMutable { .. } => match self.mut_ops_to_replace.get(&self.cur_index) {
                Some(What::Remove) => None,
                Some(What::ReadSsa(_)) | Some(What::BecomeDecl) => {
                    unreachable!("cannot convert mut decl to read or decl")
                }
                None => Some(stmt),
            },
            ir::Stmt::WriteMutable {
                ref target,
                ref val,
            } => match self.mut_ops_to_replace.get(&self.cur_index) {
                Some(What::Remove) => None,
                Some(What::BecomeDecl) => Some(ir::Stmt::DeclareMutable {
                    target: target.clone(),
                    val: val.clone(),
                }),
                Some(What::ReadSsa(_)) => unreachable!("cannot convert mut write to read"),
                None => Some(stmt),
            },
            _ => Some(stmt),
        }
    }
}
