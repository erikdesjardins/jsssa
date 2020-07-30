use std::mem;

use swc_ecma_ast as ast;
use swc_ecma_visit::{Fold, FoldWith};

/// Converts if-statements where both sides assign to the same variable into conditional expressions.
pub struct If2Cond;

impl Fold for If2Cond {
    fn fold_stmt(&mut self, stmt: ast::Stmt) -> ast::Stmt {
        let stmt = stmt.fold_children_with(self);

        match stmt {
            ast::Stmt::If(ast::IfStmt {
                span,
                test,
                mut cons,
                alt: Some(mut alt),
            }) => match (cons.as_mut(), alt.as_mut()) {
                (
                    ast::Stmt::Expr(ast::ExprStmt {
                        span: _,
                        expr: cons_expr,
                    }),
                    ast::Stmt::Expr(ast::ExprStmt {
                        span: _,
                        expr: alt_expr,
                    }),
                ) => match (cons_expr.as_mut(), alt_expr.as_mut()) {
                    (
                        ast::Expr::Assign(ast::AssignExpr {
                            span: _,
                            op: ast::AssignOp::Assign,
                            left: ast::PatOrExpr::Pat(left_pat),
                            right: left_val,
                        }),
                        ast::Expr::Assign(ast::AssignExpr {
                            span: _,
                            op: ast::AssignOp::Assign,
                            left: ast::PatOrExpr::Pat(right_pat),
                            right: right_val,
                        }),
                    ) => match (left_pat.as_mut(), right_pat.as_mut()) {
                        (ast::Pat::Ident(left_ident), ast::Pat::Ident(right_ident))
                            if left_ident.sym == right_ident.sym =>
                        {
                            ast::Stmt::Expr(ast::ExprStmt {
                                span,
                                expr: Box::new(ast::Expr::Assign(ast::AssignExpr {
                                    span,
                                    op: ast::AssignOp::Assign,
                                    left: ast::PatOrExpr::Pat(mem::replace(
                                        left_pat,
                                        Box::new(ast::Pat::Invalid(ast::Invalid { span })),
                                    )),
                                    right: Box::new(ast::Expr::Cond(ast::CondExpr {
                                        span,
                                        test,
                                        cons: mem::replace(
                                            left_val,
                                            Box::new(ast::Expr::Invalid(ast::Invalid { span })),
                                        ),
                                        alt: mem::replace(
                                            right_val,
                                            Box::new(ast::Expr::Invalid(ast::Invalid { span })),
                                        ),
                                    })),
                                })),
                            })
                        }
                        _ => ast::Stmt::If(ast::IfStmt {
                            span,
                            test,
                            cons,
                            alt: Some(alt),
                        }),
                    },
                    _ => ast::Stmt::If(ast::IfStmt {
                        span,
                        test,
                        cons,
                        alt: Some(alt),
                    }),
                },
                _ => ast::Stmt::If(ast::IfStmt {
                    span,
                    test,
                    cons,
                    alt: Some(alt),
                }),
            },
            _ => stmt,
        }
    }
}
