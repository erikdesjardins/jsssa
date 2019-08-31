use std::collections::{HashMap, HashSet};

use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Remove arrays and objects which are only written to.
///
/// Does not profit from multiple passes.
/// May profit from DCE running first; may create opportunities for DCE.
#[derive(Debug, Default)]
pub struct Objects {
    objects_to_remove: HashSet<ir::WeakRef<ir::Ssa>>,
}

#[derive(Debug, Default)]
struct CollectObjWriteInfo<'a> {
    obj_ops: HashMap<&'a ir::Ref<ir::Ssa>, State>,
    last_use_was_safe: bool,
}

#[derive(Debug)]
enum State {
    WriteOnly,
    Invalid,
}

impl<'a> Visitor<'a> for CollectObjWriteInfo<'a> {
    fn visit(&mut self, stmt: &'a ir::Stmt) {
        self.last_use_was_safe = false;

        match stmt {
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::Object { .. },
            } => {
                self.obj_ops.entry(target).or_insert(State::WriteOnly);
            }
            ir::Stmt::WriteMember { obj, prop, val } => {
                self.last_use_was_safe = true;
                let _ = obj;
                self.obj_ops.insert(prop, State::Invalid);
                self.obj_ops.insert(val, State::Invalid);
            }
            _ => {}
        }
    }

    fn visit_ref_use(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
        if !self.last_use_was_safe {
            self.obj_ops.insert(ref_, State::Invalid);
        }
    }
}

impl Folder for Objects {
    type Output = Option<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut collector = CollectObjWriteInfo::default();
            collector.run_visitor(&block);
            self.objects_to_remove = collector
                .obj_ops
                .into_iter()
                .filter_map(|(ref_, state)| match state {
                    State::WriteOnly => Some(ref_.weak()),
                    State::Invalid => None,
                })
                .collect();
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                target: ref obj,
                expr: ir::Expr::Object { .. },
            }
            | ir::Stmt::WriteMember {
                ref obj,
                prop: _,
                val: _,
            } if self.objects_to_remove.contains(&obj.weak()) => None,
            _ => Some(stmt),
        }
    }
}
