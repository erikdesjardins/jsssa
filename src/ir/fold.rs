use crate::ir;

pub trait Folder {
    type Output: IntoIterator<Item = ir::Stmt>;

    fn wrap_scope<R>(&mut self, enter: impl FnOnce(&mut Self) -> R) -> R {
        enter(self)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output;
}

pub trait RunFolder: Folder {
    fn run_folder(&mut self, ir: ir::Block) -> ir::Block;
}

impl<F: Folder> RunFolder for F {
    fn run_folder(&mut self, ir: ir::Block) -> ir::Block {
        self.wrap_scope(|this| {
            let ir::Block(children) = ir;

            let folded_children = children
                .into_iter()
                .flat_map(|child| {
                    this.fold(child)
                        .into_iter()
                        .map(|stmt| match stmt {
                            ir::Stmt::Expr { target, expr } => ir::Stmt::Expr {
                                target,
                                expr: match expr {
                                    ir::Expr::Function { kind, body } => ir::Expr::Function {
                                        kind,
                                        body: this.run_folder(body),
                                    },
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
                                    | ir::Expr::Argument { .. } => expr,
                                },
                            },
                            ir::Stmt::Loop { body } => ir::Stmt::Loop {
                                body: this.run_folder(body),
                            },
                            ir::Stmt::ForEach { kind, init, body } => ir::Stmt::ForEach {
                                kind,
                                init,
                                body: this.run_folder(body),
                            },
                            ir::Stmt::IfElse { cond, cons, alt } => ir::Stmt::IfElse {
                                cond,
                                cons: this.run_folder(cons),
                                alt: this.run_folder(alt),
                            },
                            ir::Stmt::Try {
                                body,
                                catch,
                                finally,
                            } => ir::Stmt::Try {
                                body: this.run_folder(body),
                                catch: this.run_folder(catch),
                                finally: Box::new(this.run_folder(*finally)),
                            },
                            ir::Stmt::DeclareMutable { .. }
                            | ir::Stmt::WriteMutable { .. }
                            | ir::Stmt::WriteGlobal { .. }
                            | ir::Stmt::WriteMember { .. }
                            | ir::Stmt::Return { .. }
                            | ir::Stmt::Throw { .. }
                            | ir::Stmt::Break
                            | ir::Stmt::Continue
                            | ir::Stmt::Debugger => stmt,
                        })
                        .collect::<Vec<_>>()
                })
                .collect();

            ir::Block(folded_children)
        })
    }
}
