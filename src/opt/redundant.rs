use std::collections::{HashMap, HashSet};
use std::mem;

use crate::anal;
use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Remove or forward redundant loads and stores of mutable vars.
///
/// Does not profit from multiple passes.
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
#[derive(Debug, Default)]
pub struct LoadStore {
    mut_ops_to_replace: HashMap<StmtIndex, What>,
    cur_index: StmtIndex,
}

type StmtIndex = u64;

#[derive(Debug)]
enum What {
    ReadSsa(ir::Ref<ir::Ssa>),
    Remove,
    BecomeDecl,
}

#[derive(Debug, Default)]
struct CollectLoadStoreInfo<'a> {
    refs_used_in_only_one_fn_scope: HashSet<&'a ir::Ref<ir::Mut>>,
    mut_ops_to_replace: HashMap<StmtIndex, What>,
    cur_index: StmtIndex,
    last_op_for_reads: HashMap<&'a ir::Ref<ir::Mut>, (StmtIndex, WriteOp<'a>)>,
    last_op_for_writes: HashMap<&'a ir::Ref<ir::Mut>, (StmtIndex, WriteOp<'a>)>,
    invalid_for_reads: Invalid<'a>,
    invalid_for_writes: Invalid<'a>,
}

#[derive(Debug, Clone)]
enum WriteOp<'a> {
    Declare(&'a ir::Ref<ir::Ssa>),
    Write(&'a ir::Ref<ir::Ssa>),
}

#[derive(Debug, Default)]
struct Invalid<'a> {
    all_refs_used_across_fn_scopes: bool,
    except: HashSet<&'a ir::Ref<ir::Mut>>,
    everything_in_parent: bool,
    in_parent: HashSet<&'a ir::Ref<ir::Mut>>,
}

impl<'a> CollectLoadStoreInfo<'a> {
    fn invalidate_from_child(&mut self, child: Self) {
        fn invalidate<'a>(
            ops: &mut HashMap<&'a ir::Ref<ir::Mut>, (StmtIndex, WriteOp<'a>)>,
            self_invalid: &mut Invalid<'a>,
            child_invalid: Invalid<'a>,
        ) {
            if child_invalid.all_refs_used_across_fn_scopes {
                self_invalid.all_refs_used_across_fn_scopes = true;
                self_invalid.except.clear();
            }
            if child_invalid.everything_in_parent {
                ops.clear();
                self_invalid.except.clear(); // pure optimization
                self_invalid.everything_in_parent = true;
                self_invalid.in_parent.clear(); // pure optimization
            } else {
                for invalid_ref in child_invalid.in_parent {
                    ops.remove(&invalid_ref);
                    self_invalid.in_parent.insert(invalid_ref);
                }
            }
        }
        invalidate(
            &mut self.last_op_for_reads,
            &mut self.invalid_for_reads,
            child.invalid_for_reads,
        );
        invalidate(
            &mut self.last_op_for_writes,
            &mut self.invalid_for_writes,
            child.invalid_for_writes,
        );
    }

    fn invalidate_everything_used_across_fn_scopes(&mut self) {
        self.invalid_for_reads.all_refs_used_across_fn_scopes = true;
        self.invalid_for_writes.all_refs_used_across_fn_scopes = true;
        self.invalid_for_reads.except.clear();
        self.invalid_for_writes.except.clear();
    }

    fn invalidate_everything_for_writes(&mut self) {
        self.last_op_for_writes.clear();
        self.invalid_for_writes.except.clear(); // pure optimization
        self.invalid_for_writes.everything_in_parent = true;
        self.invalid_for_writes.in_parent.clear(); // pure optimization
    }

    fn invalidate_local_only(&mut self) {
        self.last_op_for_reads.clear();
        self.last_op_for_writes.clear();
    }

    fn set_last_op(&mut self, target: &'a ir::Ref<ir::Mut>, op: (StmtIndex, WriteOp<'a>)) {
        self.last_op_for_reads.insert(target, op.clone());
        self.last_op_for_writes.insert(target, op);
        self.invalid_for_reads.except.insert(target);
        self.invalid_for_writes.except.insert(target);
        self.invalid_for_reads.in_parent.insert(target);
        self.invalid_for_writes.in_parent.insert(target);
    }

    fn get_last_op<'op>(
        &self,
        ref_: &&'a ir::Ref<ir::Mut>,
        ops: &'op HashMap<&'a ir::Ref<ir::Mut>, (StmtIndex, WriteOp<'a>)>,
        invalid: &Invalid<'a>,
    ) -> Option<&'op (StmtIndex, WriteOp<'a>)> {
        let op = ops.get(ref_)?;
        if invalid.all_refs_used_across_fn_scopes
            && !self.refs_used_in_only_one_fn_scope.contains(ref_)
            && !invalid.except.contains(ref_)
        {
            None
        } else {
            Some(op)
        }
    }

    fn declare_mut(&mut self, target: &'a ir::Ref<ir::Mut>, val: &'a ir::Ref<ir::Ssa>) {
        let op = (self.cur_index, WriteOp::Declare(val));
        self.set_last_op(target, op);
    }

    fn write_mut(&mut self, target: &'a ir::Ref<ir::Mut>, val: &'a ir::Ref<ir::Ssa>) {
        let op = match self.get_last_op(&target, &self.last_op_for_writes, &self.invalid_for_writes)
        {
            // write -> write (decl)
            Some((declare_index, WriteOp::Declare(_))) => {
                self.mut_ops_to_replace.insert(*declare_index, What::Remove);
                self.mut_ops_to_replace
                    .insert(self.cur_index, What::BecomeDecl);
                (self.cur_index, WriteOp::Declare(val))
            }
            // write -> write
            Some((write_index, WriteOp::Write(_))) => {
                self.mut_ops_to_replace.insert(*write_index, What::Remove);
                (self.cur_index, WriteOp::Write(val))
            }
            None => (self.cur_index, WriteOp::Write(val)),
        };
        self.set_last_op(target, op);
    }

    fn read_mut(&mut self, _target: &'a ir::Ref<ir::Ssa>, source: &'a ir::Ref<ir::Mut>) {
        match self.get_last_op(&source, &self.last_op_for_reads, &self.invalid_for_reads) {
            // write -> read
            Some((_, WriteOp::Declare(val))) | Some((_, WriteOp::Write(val))) => {
                self.mut_ops_to_replace
                    .insert(self.cur_index, What::ReadSsa((*val).clone()));
            }
            None => {}
        }
    }
}

impl<'a> Visitor<'a> for CollectLoadStoreInfo<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            self.refs_used_in_only_one_fn_scope =
                anal::refs::used_in_only_one_fn_scope(&block).collect();
        }

        match ty {
            ScopeTy::Function => {
                // function scopes are analyzed separately
                let mut inner = Self::default();
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                let r = enter(&mut inner, block);
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel => {
                // r->r and w->r can go into scopes, but not w->w (since it might not execute)
                let mut inner = Self::default();
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                // todo avoid these clones
                inner.last_op_for_reads = self.last_op_for_reads.clone();
                inner.invalid_for_reads.all_refs_used_across_fn_scopes =
                    self.invalid_for_reads.all_refs_used_across_fn_scopes;
                inner.invalid_for_reads.except = self.invalid_for_reads.except.clone();
                let r = enter(&mut inner, block);
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                // invalidate any vars written in the inner scope
                self.invalidate_from_child(inner);
                r
            }
            ScopeTy::Nonlinear => {
                // forwarding can happen across nonlinear scopes, but not into them
                let mut inner = Self::default();
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                let r = enter(&mut inner, block);
                mem::swap(
                    &mut inner.refs_used_in_only_one_fn_scope,
                    &mut self.refs_used_in_only_one_fn_scope,
                );
                mem::swap(&mut inner.mut_ops_to_replace, &mut self.mut_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                // invalidate any vars written in the inner scope
                self.invalidate_from_child(inner);
                r
            }
        }
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        self.cur_index += 1;
        match stmt {
            ir::Stmt::Expr { target, expr } => match expr {
                ir::Expr::ReadMutable { source } => self.read_mut(target, source),
                ir::Expr::Yield { .. } | ir::Expr::Await { .. } | ir::Expr::Call { .. } => {
                    self.invalidate_everything_used_across_fn_scopes()
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
            ir::Stmt::Return { .. }
            | ir::Stmt::Throw { .. }
            | ir::Stmt::Break { .. }
            | ir::Stmt::Continue { .. } => self.invalidate_everything_for_writes(),
            ir::Stmt::SwitchCase { .. } => self.invalidate_local_only(),
            ir::Stmt::WriteGlobal { .. }
            | ir::Stmt::WriteMember { .. }
            | ir::Stmt::Debugger { .. }
            | ir::Stmt::Label { .. }
            | ir::Stmt::Loop { .. }
            | ir::Stmt::ForEach { .. }
            | ir::Stmt::IfElse { .. }
            | ir::Stmt::Switch { .. }
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
                target,
                expr: ir::Expr::ReadMutable { source },
            } => match self.mut_ops_to_replace.remove(&self.cur_index) {
                Some(What::ReadSsa(ssa_ref)) => Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Read { source: ssa_ref },
                }),
                Some(What::Remove) | Some(What::BecomeDecl) => {
                    unreachable!("cannot remove/convert mut read to decl")
                }
                None => Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::ReadMutable { source },
                }),
            },
            ir::Stmt::DeclareMutable { target, val } => {
                match self.mut_ops_to_replace.remove(&self.cur_index) {
                    Some(What::Remove) => None,
                    Some(What::ReadSsa(_)) | Some(What::BecomeDecl) => {
                        unreachable!("cannot convert mut decl to read or decl")
                    }
                    None => Some(ir::Stmt::DeclareMutable { target, val }),
                }
            }
            ir::Stmt::WriteMutable { target, val } => {
                match self.mut_ops_to_replace.remove(&self.cur_index) {
                    Some(What::Remove) => None,
                    Some(What::BecomeDecl) => Some(ir::Stmt::DeclareMutable { target, val }),
                    Some(What::ReadSsa(_)) => unreachable!("cannot convert mut write to read"),
                    None => Some(ir::Stmt::WriteMutable { target, val }),
                }
            }
            _ => Some(stmt),
        }
    }
}
