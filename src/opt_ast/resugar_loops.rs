use swc_ecma_ast as ast;
use swc_ecma_visit::{Fold, FoldWith};

/// Moves loop conditions back into loop headers.
pub struct ResugarLoops;

impl Fold for ResugarLoops {
    fn fold_for_stmt(&mut self, stmt: ast::ForStmt) -> ast::ForStmt {
        let mut stmt = stmt.fold_children_with(self);

        match &mut stmt {
            ast::ForStmt {
                init: _,
                test: for_test @ None,
                update: _,
                body,
                span: _,
            } => match body.as_mut() {
                ast::Stmt::Block(ast::BlockStmt { stmts, span: _ }) => match stmts.get(0) {
                    Some(ast::Stmt::If(ast::IfStmt {
                        test: _,
                        cons,
                        alt: Some(alt),
                        span: _,
                    })) => match (cons.as_ref(), alt.as_ref()) {
                        (
                            ast::Stmt::Empty(ast::EmptyStmt { span: _ }),
                            ast::Stmt::Break(ast::BreakStmt {
                                label: None,
                                span: _,
                            }),
                        ) => match stmts.remove(0) {
                            ast::Stmt::If(ast::IfStmt { test, .. }) => {
                                *for_test = Some(test);
                                stmt
                            }
                            _ => unreachable!(),
                        },
                        _ => stmt,
                    },
                    _ => stmt,
                },
                _ => stmt,
            },
            _ => stmt,
        }
    }
}
