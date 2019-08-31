use std::collections::HashMap;

use crate::ir;
use crate::ir::traverse::{visit_with, Folder, ScopeTy};

/// Forward `ir::Expr::Read` to the source SSA ref.
///
/// Does not profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
#[derive(Debug, Default)]
pub struct Reads {
    ssa_remappings: HashMap<ir::WeakRef<ir::Ssa>, What>,
    prev_expr_if_moving_to_next: Option<ir::Expr>,
}

#[derive(Debug)]
enum What {
    MoveToNext,
    ForwardTo(ir::Ref<ir::Ssa>),
}

impl Folder for Reads {
    type Output = Option<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut prev_ref_if_single_use = None;
            visit_with(&block, |stmt: &ir::Stmt| {
                prev_ref_if_single_use = match stmt {
                    ir::Stmt::Expr { target, expr } => {
                        if let ir::Expr::Read { source } = expr {
                            if prev_ref_if_single_use == Some(source) {
                                // move prev ref down to here, removing this read
                                self.ssa_remappings.insert(source.weak(), What::MoveToNext);
                            } else {
                                // forward uses of this read to its source, or its source's source
                                match self.ssa_remappings.get(&source.weak()) {
                                    Some(What::MoveToNext) => {
                                        unreachable!("MoveToNext should imply single use")
                                    }
                                    Some(What::ForwardTo(orig_ref)) => {
                                        let orig_ref = orig_ref.clone();
                                        self.ssa_remappings
                                            .insert(target.weak(), What::ForwardTo(orig_ref));
                                    }
                                    None => {
                                        self.ssa_remappings
                                            .insert(target.weak(), What::ForwardTo(source.clone()));
                                    }
                                }
                            }
                        }
                        if target.used().is_once() {
                            Some(target)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
            });
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr { target, mut expr } => {
                match self.prev_expr_if_moving_to_next.take() {
                    Some(prev_expr) => {
                        match &expr {
                            ir::Expr::Read { source: _ } => {}
                            e => unreachable!("target isn't a read: {:?}", e),
                        }
                        expr = prev_expr;
                    }
                    None => {}
                }

                match self.ssa_remappings.get(&target.weak()) {
                    Some(What::MoveToNext) => {
                        self.prev_expr_if_moving_to_next = Some(expr);
                        None
                    }
                    Some(What::ForwardTo(_)) | None => {
                        // keep this expr, since it may have non-forwarded uses
                        Some(ir::Stmt::Expr { target, expr })
                    }
                }
            }
            _ => {
                assert!(self.prev_expr_if_moving_to_next.is_none());
                Some(stmt)
            }
        }
    }

    fn fold_ref_use(&mut self, ref_: ir::Ref<ir::Ssa>) -> ir::Ref<ir::Ssa> {
        match self.ssa_remappings.get(&ref_.weak()) {
            Some(What::MoveToNext) => unreachable!("MoveToNext should imply single use"),
            Some(What::ForwardTo(orig_ref)) => orig_ref.clone(),
            None => ref_,
        }
    }
}
