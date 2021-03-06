use crate::ir;
use crate::ir::traverse::{Folder, ScopeTy};

/// Dead code elimination.
///
/// May profit from multiple passes.
#[derive(Debug, Default)]
pub struct Dce {
    dropping_after_jump: bool,
}

impl Folder for Dce {
    type Output = Option<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        _: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        let r = enter(self, block);
        // stop dropping when we leave the current scope...
        self.dropping_after_jump = false;
        r
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        // ...or when encountering a case statement
        if let ir::Stmt::SwitchCase { .. } = &stmt {
            self.dropping_after_jump = false;
        }

        if self.dropping_after_jump {
            return None;
        }

        match stmt {
            ir::Stmt::Expr {
                ref target,
                ref expr,
            } if target.used().is_never() => match expr {
                ir::Expr::Function { .. }
                | ir::Expr::Bool { .. }
                | ir::Expr::Number { .. }
                | ir::Expr::String { .. }
                | ir::Expr::Null
                | ir::Expr::Undefined
                | ir::Expr::This
                | ir::Expr::Read { .. }
                | ir::Expr::ReadMutable { .. }
                | ir::Expr::ReadGlobal { .. }
                | ir::Expr::Array { .. }
                | ir::Expr::Object { .. }
                | ir::Expr::RegExp { .. }
                | ir::Expr::Unary { .. }
                | ir::Expr::Binary { .. }
                | ir::Expr::CurrentFunction
                | ir::Expr::Argument { .. } => None,
                ir::Expr::ReadMember { .. }
                | ir::Expr::Delete { .. }
                | ir::Expr::Yield { .. }
                | ir::Expr::Await { .. }
                | ir::Expr::Call { .. } => Some(stmt),
            },
            ir::Stmt::DeclareMutable { ref target, val: _ } if target.used().is_never() => None,
            ir::Stmt::Return { .. }
            | ir::Stmt::Throw { .. }
            | ir::Stmt::Break { .. }
            | ir::Stmt::Continue { .. } => {
                self.dropping_after_jump = true;
                Some(stmt)
            }
            ir::Stmt::IfElse {
                cond: _,
                ref cons,
                ref alt,
            } if cons.0.is_empty() && alt.0.is_empty() => None,
            ir::Stmt::Try {
                ref body,
                catch: _,
                ref finally,
            } if body.0.is_empty() && finally.0.is_empty() => None,
            _ => Some(stmt),
        }
    }
}
