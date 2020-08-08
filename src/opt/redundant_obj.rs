use std::collections::{HashMap, HashSet};
use std::mem;

use crate::collections::SmallMap;
use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Remove or forward redundant loads and stores of object properties.
///
/// May profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for read forwarding.
///
/// Performs the following transformations:
///
/// Read-to-read forwarding:
///   something = { z: _1 }
///   /* stuff */
///   x = something.z
///   y = something.z
/// ->
///   something = { z: _1 }
///   /* stuff */
///   x = something.z
///   y = x
///
/// Write-to-read forwarding:
///   something = { z: _1 }
///   /* stuff */
///   x = 1 + 1
///   something.z <- x
///   y = something.z
/// ->
///   something = { z: _1 }
///   /* stuff */
///   x = 1 + 1
///   something.z <- x
///   y = x
///
/// Dead write elimination:
///   something = { z: _1 }
///   /* stuff */
///   something.z <- x
///   something.z <- y
/// ->
///   something = { z: _1 }
///   /* stuff */
///   something.z <- y
///
#[derive(Debug, Default)]
pub struct LoadStore {
    obj_ops_to_replace: HashMap<StmtIndex, What>,
    cur_index: StmtIndex,
}

type StmtIndex = u64;

#[derive(Debug)]
enum What {
    ReadSsa(ir::Ref<ir::Ssa>),
    Remove,
}

#[derive(Debug, Default)]
struct CollectLoadStoreInfo<'a> {
    obj_ops_to_replace: HashMap<StmtIndex, What>,
    cur_index: StmtIndex,
    known_strings: HashMap<&'a ir::Ref<ir::Ssa>, &'a str>,
    for_reads: Ops<'a>,
    for_writes: Ops<'a>,
}

#[derive(Debug, Default)]
struct Ops<'a> {
    last_op_for_prop: HashMap<&'a str, SmallMap<&'a ir::Ref<ir::Ssa>, (StmtIndex, Op<'a>)>>,
    invalid_in_parent: Invalid<'a>,
}

#[derive(Debug, Clone)]
enum Op<'a> {
    Declare(&'a ir::Ref<ir::Ssa>),
    Write(&'a ir::Ref<ir::Ssa>),
    Read(&'a ir::Ref<ir::Ssa>),
}

#[derive(Debug)]
enum Invalid<'a> {
    All,
    Props(HashSet<&'a str>),
}

impl Default for Invalid<'_> {
    fn default() -> Self {
        Self::Props(Default::default())
    }
}

impl<'a> Invalid<'a> {
    fn insert_prop(&mut self, prop: &'a str) {
        match self {
            Self::All => {
                // all objects already invalid
            }
            Self::Props(props) => {
                props.insert(prop);
            }
        }
    }
}

impl<'a> CollectLoadStoreInfo<'a> {
    fn invalidate_from_child(&mut self, child: Self) {
        fn invalidate<'a>(this: &mut Ops<'a>, child: Ops<'a>) {
            match child.invalid_in_parent {
                Invalid::All => {
                    this.last_op_for_prop.clear();
                    this.invalid_in_parent = Invalid::All;
                }
                Invalid::Props(props) => {
                    for prop in props {
                        this.last_op_for_prop.remove(&prop);
                        this.invalid_in_parent.insert_prop(prop);
                    }
                }
            }
        }
        invalidate(&mut self.for_reads, child.for_reads);
        invalidate(&mut self.for_writes, child.for_writes);
    }

    fn invalidate_everything(&mut self) {
        fn invalidate(this: &mut Ops<'_>) {
            this.last_op_for_prop.clear();
            this.invalid_in_parent = Invalid::All;
        }
        invalidate(&mut self.for_reads);
        invalidate(&mut self.for_writes);
    }

    fn invalidate_everything_for_writes(&mut self) {
        fn invalidate(this: &mut Ops<'_>) {
            this.last_op_for_prop.clear();
            this.invalid_in_parent = Invalid::All;
        }
        invalidate(&mut self.for_writes);
    }

    fn invalidate_current_scope(&mut self) {
        fn invalidate(this: &mut Ops<'_>) {
            this.last_op_for_prop.clear();
        }
        invalidate(&mut self.for_reads);
        invalidate(&mut self.for_writes);
    }

    fn set_last_write_op(
        &mut self,
        obj: &'a ir::Ref<ir::Ssa>,
        prop: &'a str,
        op: (StmtIndex, Op<'a>),
    ) {
        assert!(matches!(op.1, Op::Declare(_) | Op::Write(_)));
        let ops = SmallMap::One(obj, op);
        self.for_reads.last_op_for_prop.insert(prop, ops.clone());
        self.for_writes.last_op_for_prop.insert(prop, ops);
        self.for_reads.invalid_in_parent.insert_prop(prop);
        self.for_writes.invalid_in_parent.insert_prop(prop);
    }

    fn set_last_read_op(
        &mut self,
        obj: &'a ir::Ref<ir::Ssa>,
        prop: &'a str,
        op: (StmtIndex, Op<'a>),
    ) {
        assert!(matches!(op.1, Op::Read(_)));
        self.for_reads
            .last_op_for_prop
            .entry(prop)
            .or_default()
            .insert(obj, op);
        self.for_writes.last_op_for_prop.remove(prop);
        self.for_writes.invalid_in_parent.insert_prop(prop);
    }

    fn get_last_op<'op>(
        &self,
        ops: &'op Ops<'a>,
        obj: &'a ir::Ref<ir::Ssa>,
        prop: &'a str,
    ) -> Option<&'op (StmtIndex, Op<'a>)> {
        ops.last_op_for_prop
            .get(prop)
            .and_then(|objs| objs.get(obj))
    }

    fn declare_prop(
        &mut self,
        obj: &'a ir::Ref<ir::Ssa>,
        prop: &'a str,
        val: &'a ir::Ref<ir::Ssa>,
    ) {
        let op = (self.cur_index, Op::Declare(val));
        self.set_last_write_op(obj, prop, op);
    }

    fn write_prop(&mut self, obj: &'a ir::Ref<ir::Ssa>, prop: &'a str, val: &'a ir::Ref<ir::Ssa>) {
        let op = match self.get_last_op(&self.for_writes, &obj, &prop) {
            // write -> write (write)
            Some((write_index, Op::Write(_))) => {
                self.obj_ops_to_replace.insert(*write_index, What::Remove);
                (self.cur_index, Op::Write(val))
            }
            // write -> write (decl: can't overwrite decl)
            Some((_, Op::Declare(_))) | Some((_, Op::Read(_))) | None => {
                (self.cur_index, Op::Write(val))
            }
        };
        self.set_last_write_op(obj, prop, op);
    }

    fn read_prop(
        &mut self,
        target: &'a ir::Ref<ir::Ssa>,
        obj: &'a ir::Ref<ir::Ssa>,
        prop: &'a str,
    ) {
        let op = match self.get_last_op(&self.for_reads, &obj, &prop) {
            // write -> read, read -> read
            Some((_, Op::Declare(val))) | Some((_, Op::Write(val))) | Some((_, Op::Read(val))) => {
                self.obj_ops_to_replace
                    .insert(self.cur_index, What::ReadSsa((*val).clone()));
                // this read will be dropped, don't add an op (this allows write-write with an intervening read that is forwarded out)
                None
            }
            None => Some((self.cur_index, Op::Read(target))),
        };
        if let Some(op) = op {
            self.set_last_read_op(obj, prop, op);
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
        match ty {
            ScopeTy::Function => {
                // function scopes are analyzed separately
                let mut inner = Self::default();
                mem::swap(&mut inner.obj_ops_to_replace, &mut self.obj_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.known_strings, &mut self.known_strings);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.obj_ops_to_replace, &mut self.obj_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.known_strings, &mut self.known_strings);
                r
            }
            ScopeTy::Normal | ScopeTy::Toplevel => {
                // r->r and w->r can go into scopes, but not w->w (since it might not execute)
                // and in particular, r->r can't go _out_ of scopes
                let mut inner = Self::default();
                mem::swap(&mut inner.obj_ops_to_replace, &mut self.obj_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.known_strings, &mut self.known_strings);
                // todo avoid these clones
                inner.for_reads.last_op_for_prop = self.for_reads.last_op_for_prop.clone();
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.obj_ops_to_replace, &mut self.obj_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.known_strings, &mut self.known_strings);
                // invalidate any vars written in the inner scope
                self.invalidate_from_child(inner);
                r
            }
            ScopeTy::Nonlinear => {
                // forwarding can happen across nonlinear scopes, but not into them
                let mut inner = Self::default();
                mem::swap(&mut inner.obj_ops_to_replace, &mut self.obj_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.known_strings, &mut self.known_strings);
                let r = enter(&mut inner, block);
                mem::swap(&mut inner.obj_ops_to_replace, &mut self.obj_ops_to_replace);
                mem::swap(&mut inner.cur_index, &mut self.cur_index);
                mem::swap(&mut inner.known_strings, &mut self.known_strings);
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
                ir::Expr::String { value } => {
                    self.known_strings.insert(target, &value);
                }
                ir::Expr::ReadMember { obj, prop } => {
                    match self.known_strings.get(prop) {
                        Some(prop) => {
                            let prop = *prop;
                            self.read_prop(target, obj, prop);
                        }
                        None => {
                            // read from unknown prop: do not drop previous writes (because it might have read them)
                            self.invalidate_everything_for_writes();
                        }
                    }
                }
                ir::Expr::Object { props } => {
                    for (kind, prop, val) in props {
                        match kind {
                            ir::PropKind::Simple => match self.known_strings.get(prop) {
                                Some(prop) => {
                                    let prop = *prop;
                                    self.declare_prop(target, prop, val);
                                }
                                None => {
                                    // write to unknown prop: invalidate
                                    self.invalidate_everything();
                                }
                            },
                            ir::PropKind::Get | ir::PropKind::Set => {
                                unimplemented!("getter/setter props not handled")
                            }
                        }
                    }
                }
                ir::Expr::Yield { .. } | ir::Expr::Await { .. } | ir::Expr::Call { .. } => {
                    self.invalidate_everything()
                }
                ir::Expr::Bool { .. }
                | ir::Expr::Number { .. }
                | ir::Expr::Null
                | ir::Expr::Undefined
                | ir::Expr::This
                | ir::Expr::Read { .. }
                | ir::Expr::ReadMutable { .. }
                | ir::Expr::ReadGlobal { .. }
                | ir::Expr::Array { .. }
                | ir::Expr::RegExp { .. }
                | ir::Expr::Unary { .. }
                | ir::Expr::Binary { .. }
                | ir::Expr::Delete { .. }
                | ir::Expr::Function { .. }
                | ir::Expr::CurrentFunction { .. }
                | ir::Expr::Argument { .. } => {}
            },
            ir::Stmt::WriteMember { obj, prop, val } => {
                match self.known_strings.get(prop) {
                    Some(prop) => {
                        let prop = *prop;
                        self.write_prop(obj, prop, val);
                    }
                    None => {
                        // write to unknown prop: invalidate
                        self.invalidate_everything();
                    }
                }
            }
            ir::Stmt::Return { .. }
            | ir::Stmt::Throw { .. }
            | ir::Stmt::Break { .. }
            | ir::Stmt::Continue { .. } => self.invalidate_everything_for_writes(),
            ir::Stmt::SwitchCase { .. } => self.invalidate_current_scope(),
            ir::Stmt::DeclareMutable { .. }
            | ir::Stmt::WriteMutable { .. }
            | ir::Stmt::WriteGlobal { .. }
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
            self.obj_ops_to_replace = collector.obj_ops_to_replace;
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        self.cur_index += 1;

        match stmt {
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::ReadMember { obj, prop },
            } => match self.obj_ops_to_replace.remove(&self.cur_index) {
                Some(What::ReadSsa(ssa_ref)) => Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Read { source: ssa_ref },
                }),
                Some(What::Remove) => unreachable!("cannot remove read"),
                None => Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::ReadMember { obj, prop },
                }),
            },
            ir::Stmt::WriteMember { obj, prop, val } => {
                match self.obj_ops_to_replace.remove(&self.cur_index) {
                    Some(What::Remove) => None,
                    Some(What::ReadSsa(_)) => unreachable!("cannot convert write to read"),
                    None => Some(ir::Stmt::WriteMember { obj, prop, val }),
                }
            }
            _ => Some(stmt),
        }
    }
}
