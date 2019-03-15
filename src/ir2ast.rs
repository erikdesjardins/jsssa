use swc_common::Span;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;
use crate::utils::P;

pub fn convert(ir: ir::Block) -> ast::Script {
    // todo perform inlining at this stage? (i.e. scan backwards for all usages)
    let body = convert_block(ir, &scope::Ir::default());
    ast::Script {
        span: span(),
        body,
        shebang: None,
    }
}

fn convert_block(block: ir::Block, parent_scope: &scope::Ir) -> Vec<ast::Stmt> {
    let mut scope = parent_scope.clone();

    let ir::Block { children } = block;

    children
        .into_iter()
        .map(|stmt| convert_stmt(stmt, &mut scope))
        .collect()
}

fn convert_stmt(stmt: ir::Stmt, scope: &mut scope::Ir) -> ast::Stmt {
    match stmt {
        ir::Stmt::Expr { target, expr } => {
            let expr = convert_expr(expr, scope);
            if target.maybe_used() {
                let name = scope.declare_ssa(target);
                ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
                    span: span(),
                    kind: ast::VarDeclKind::Var,
                    decls: vec![ast::VarDeclarator {
                        span: span(),
                        name: ast::Pat::Ident(ast::Ident {
                            span: span(),
                            sym: name,
                            type_ann: None,
                            optional: false,
                        }),
                        init: Some(P(expr)),
                        definite: false,
                    }],
                    declare: false,
                }))
            } else {
                ast::Stmt::Expr(P(expr))
            }
        }
        ir::Stmt::WriteBinding { target, val } => match scope.get_mutable(&target) {
            Some(existing_name) => ast::Stmt::Expr(P(ast::Expr::Assign(ast::AssignExpr {
                span: span(),
                op: ast::AssignOp::Assign,
                left: ast::PatOrExpr::Pat(P(ast::Pat::Ident(ast::Ident {
                    span: span(),
                    sym: existing_name,
                    type_ann: None,
                    optional: false,
                }))),
                right: P(read_ssa_to_expr(val, scope)),
            }))),
            None => {
                let name = scope.declare_mutable(target);
                ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
                    span: span(),
                    kind: ast::VarDeclKind::Var,
                    decls: vec![ast::VarDeclarator {
                        span: span(),
                        name: ast::Pat::Ident(ast::Ident {
                            span: span(),
                            sym: name,
                            type_ann: None,
                            optional: false,
                        }),
                        init: Some(P(read_ssa_to_expr(val, scope))),
                        definite: false,
                    }],
                    declare: false,
                }))
            }
        },
        ir::Stmt::WriteGlobal { target, val } => unimplemented!(),
        ir::Stmt::WriteMember { obj, prop, val } => unimplemented!(),
        ir::Stmt::Return { val } => ast::Stmt::Return(ast::ReturnStmt {
            span: span(),
            arg: Some(P(read_ssa_to_expr(val, scope))),
        }),
        ir::Stmt::Throw { val } => ast::Stmt::Throw(ast::ThrowStmt {
            span: span(),
            arg: P(read_ssa_to_expr(val, scope)),
        }),
        ir::Stmt::Break => ast::Stmt::Break(ast::BreakStmt {
            span: span(),
            label: None,
        }),
        ir::Stmt::Continue => ast::Stmt::Continue(ast::ContinueStmt {
            span: span(),
            label: None,
        }),
        ir::Stmt::Debugger => ast::Stmt::Debugger(ast::DebuggerStmt { span: span() }),
        ir::Stmt::Block { body } => unimplemented!(),
        ir::Stmt::Loop { body } => unimplemented!(),
        ir::Stmt::For {
            kind,
            var,
            init,
            body,
        } => unimplemented!(),
        ir::Stmt::IfElse { cond, cons, alt } => unimplemented!(),
        ir::Stmt::Try {
            body,
            catch,
            finally,
        } => unimplemented!(),
    }
}

fn convert_expr(expr: ir::Expr, scope: &scope::Ir) -> ast::Expr {
    unimplemented!()
}

fn read_ssa_to_expr(ssa_ref: ir::Ref<ir::SSA>, scope: &scope::Ir) -> ast::Expr {
    let name = match scope.get_ssa(&ssa_ref) {
        Some(name) => name,
        None => unreachable!("reading from undeclared SSA ref"),
    };
    ast::Expr::Ident(ast::Ident {
        span: span(),
        sym: name,
        type_ann: None,
        optional: false,
    })
}

fn span() -> Span {
    // todo make sourcemaps work by wiring this through from the original AST
    Span::default()
}
