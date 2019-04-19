use std::collections::HashMap;
use std::iter;

use crate::ir;
use crate::ir::traverse::{visit_with, Folder, ScopeTy};

/// Forward `ir::Expr::Read` to the source SSA ref.
///
/// Does not profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
#[derive(Default)]
pub struct Reads {
    ssa_remappings: HashMap<ir::WeakRef<ir::Ssa>, ir::Ref<ir::Ssa>>,
}

impl Folder for Reads {
    type Output = iter::Once<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            visit_with(&block, |stmt: &ir::Stmt| match stmt {
                ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Read { source },
                } => match self.ssa_remappings.get(&source.weak()) {
                    Some(orig_ref) => {
                        let orig_ref = orig_ref.clone();
                        self.ssa_remappings.insert(target.weak(), orig_ref);
                    }
                    None => {
                        self.ssa_remappings.insert(target.weak(), source.clone());
                    }
                },
                _ => {}
            });
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        iter::once(stmt)
    }

    fn fold_ref_use(&mut self, ref_: ir::Ref<ir::Ssa>) -> ir::Ref<ir::Ssa> {
        match self.ssa_remappings.get(&ref_.weak()) {
            Some(orig_ref) => orig_ref.clone(),
            None => ref_,
        }
    }
}
