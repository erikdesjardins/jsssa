use crate::ir;

pub trait Visitor {
    fn wrap_scope<R>(&mut self, enter: impl FnOnce(&mut Self) -> R) -> R {
        enter(self)
    }

    fn visit(&mut self, stmt: &ir::Stmt);
}

pub trait VisitorRun: Visitor {
    fn run_visitor(&mut self, ir: &ir::Block);
}

pub struct VisitFn<F>(F);

pub fn visitor_fn<F: FnMut(&ir::Stmt)>(f: F) -> VisitFn<F> {
    VisitFn(f)
}

impl<F: FnMut(&ir::Stmt)> Visitor for VisitFn<F> {
    fn visit(&mut self, stmt: &ir::Stmt) {
        (self.0)(stmt);
    }
}

impl<V: Visitor> VisitorRun for V {
    fn run_visitor(&mut self, ir: &ir::Block) {
        self.wrap_scope(|this| {
            let ir::Block(children) = ir;

            for child in children {
                this.visit(child);

                match child {
                    ir::Stmt::Expr { target: _, expr } => match expr {
                        ir::Expr::Function { kind: _, body } => {
                            this.run_visitor(body);
                        }
                        ir::Expr::Bool { .. }
                        | ir::Expr::Number { .. }
                        | ir::Expr::String { .. }
                        | ir::Expr::Null
                        | ir::Expr::Undefined
                        | ir::Expr::This
                        | ir::Expr::Read { .. }
                        | ir::Expr::ReadMutable { .. }
                        | ir::Expr::ReadGlobal { .. }
                        | ir::Expr::ReadMember { .. }
                        | ir::Expr::Array { .. }
                        | ir::Expr::Object { .. }
                        | ir::Expr::RegExp { .. }
                        | ir::Expr::Unary { .. }
                        | ir::Expr::Binary { .. }
                        | ir::Expr::Delete { .. }
                        | ir::Expr::Yield { .. }
                        | ir::Expr::Await { .. }
                        | ir::Expr::Call { .. }
                        | ir::Expr::CurrentFunction
                        | ir::Expr::Argument { .. } => {}
                    },
                    ir::Stmt::Block { body } => {
                        this.run_visitor(body);
                    }
                    ir::Stmt::Loop { body } => {
                        this.run_visitor(body);
                    }
                    ir::Stmt::ForEach {
                        kind: _,
                        init: _,
                        body,
                    } => {
                        this.run_visitor(body);
                    }
                    ir::Stmt::IfElse { cond: _, cons, alt } => {
                        this.run_visitor(cons);
                        this.run_visitor(alt);
                    }
                    ir::Stmt::Try {
                        body,
                        catch,
                        finally,
                    } => {
                        this.run_visitor(body);
                        this.run_visitor(catch);
                        this.run_visitor(finally);
                    }
                    ir::Stmt::DeclareMutable { .. }
                    | ir::Stmt::WriteMutable { .. }
                    | ir::Stmt::WriteGlobal { .. }
                    | ir::Stmt::WriteMember { .. }
                    | ir::Stmt::Return { .. }
                    | ir::Stmt::Throw { .. }
                    | ir::Stmt::Break
                    | ir::Stmt::Continue
                    | ir::Stmt::Debugger => {}
                }
            }
        })
    }
}
