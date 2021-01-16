use std::mem;

use swc_ecma_ast as ast;
use swc_ecma_visit::{Fold, FoldWith};

/// Converts if-statements where both sides assign to the same variable into conditional expressions.
pub struct If2Cond;

impl Fold for If2Cond {
    fn fold_stmt(&mut self, stmt: ast::Stmt) -> ast::Stmt {
        let mut stmt = stmt.fold_children_with(self);

        match &mut stmt {
            ast::Stmt::If(ast::IfStmt {
                test,
                cons,
                alt: Some(alt),
                span,
            }) => match (cons.as_mut(), alt.as_mut()) {
                (
                    ast::Stmt::Expr(ast::ExprStmt {
                        expr: cons_expr,
                        span: _,
                    }),
                    ast::Stmt::Expr(ast::ExprStmt {
                        expr: alt_expr,
                        span: _,
                    }),
                ) => match (cons_expr.as_mut(), alt_expr.as_mut()) {
                    (
                        ast::Expr::Assign(ast::AssignExpr {
                            op: ast::AssignOp::Assign,
                            left: ast::PatOrExpr::Pat(left_pat),
                            right: left_val,
                            span: _,
                        }),
                        ast::Expr::Assign(ast::AssignExpr {
                            op: ast::AssignOp::Assign,
                            left: ast::PatOrExpr::Pat(right_pat),
                            right: right_val,
                            span: _,
                        }),
                    ) => match (left_pat.as_ref(), right_pat.as_ref()) {
                        (ast::Pat::Ident(left_ident), ast::Pat::Ident(right_ident))
                            if left_ident.sym == right_ident.sym =>
                        {
                            ast::Stmt::Expr(ast::ExprStmt {
                                span: *span,
                                expr: Box::new(ast::Expr::Assign(ast::AssignExpr {
                                    span: *span,
                                    op: ast::AssignOp::Assign,
                                    left: ast::PatOrExpr::Pat(mem::replace(
                                        left_pat,
                                        Box::new(ast::Pat::Invalid(ast::Invalid { span: *span })),
                                    )),
                                    right: Box::new(ast::Expr::Cond(ast::CondExpr {
                                        span: *span,
                                        test: mem::replace(
                                            test,
                                            Box::new(ast::Expr::Invalid(ast::Invalid {
                                                span: *span,
                                            })),
                                        ),
                                        cons: mem::replace(
                                            left_val,
                                            Box::new(ast::Expr::Invalid(ast::Invalid {
                                                span: *span,
                                            })),
                                        ),
                                        alt: mem::replace(
                                            right_val,
                                            Box::new(ast::Expr::Invalid(ast::Invalid {
                                                span: *span,
                                            })),
                                        ),
                                    })),
                                })),
                            })
                        }
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
