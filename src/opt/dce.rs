use crate::ir;
use crate::ir::fold::Folder;

pub struct Dce;

impl Folder for Dce {
    type Output = Option<ir::Stmt>;

    fn fold(&mut self, stmt: ir::Stmt) -> Option<ir::Stmt> {
        match stmt {
            ir::Stmt::Expr { target, expr } => {
                if !target.maybe_used() {
                    match expr {
                        ir::Expr::Function { .. }
                        | ir::Expr::Bool { .. }
                        | ir::Expr::Number { .. }
                        | ir::Expr::String { .. }
                        | ir::Expr::Null
                        | ir::Expr::Undefined
                        | ir::Expr::This
                        | ir::Expr::Read { .. }
                        | ir::Expr::ReadMutable { .. }
                        | ir::Expr::Array { .. }
                        | ir::Expr::Object { .. }
                        | ir::Expr::RegExp { .. }
                        | ir::Expr::Unary { .. }
                        | ir::Expr::Binary { .. }
                        | ir::Expr::CurrentFunction
                        | ir::Expr::Argument { .. } => None,
                        ir::Expr::ReadGlobal { .. }
                        | ir::Expr::ReadMember { .. }
                        | ir::Expr::Delete { .. }
                        | ir::Expr::Yield { .. }
                        | ir::Expr::Await { .. }
                        | ir::Expr::Call { .. } => Some(ir::Stmt::Expr { target, expr }),
                    }
                } else {
                    Some(ir::Stmt::Expr { target, expr })
                }
            }
            ir::Stmt::DeclareMutable { ref target, val: _ } if !target.maybe_used() => None,
            _ => Some(stmt),
        }
    }
}
