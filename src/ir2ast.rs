use swc_common::Span;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;
use crate::utils::P;

fn temp_span() -> Span {
    // todo
    unimplemented!()
}

pub fn convert(ir: ir::Block) -> ast::Script {
    // todo perform inlining at this stage? (i.e. scan backwards for all usages)
    let body = convert_block(ir, &scope::Ir::default());
    ast::Script {
        span: temp_span(),
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
        ir::Stmt::Expr(write_ref, expr) => {
            let expr = convert_expr(expr, scope);
            if write_ref.maybe_used() {
                let name = scope.declare_ssa(write_ref);
                ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
                    span: temp_span(),
                    kind: ast::VarDeclKind::Var,
                    decls: vec![ast::VarDeclarator {
                        span: temp_span(),
                        name: ast::Pat::Ident(ast::Ident {
                            span: temp_span(),
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
        ir::Stmt::WriteBinding(target_ref, source_ref) => {
            let expr = ssa_to_expr(source_ref, scope);
            match scope.get_mutable(&target_ref) {
                Some(existing_name) => ast::Stmt::Expr(P(ast::Expr::Assign(ast::AssignExpr {
                    span: temp_span(),
                    op: ast::AssignOp::Assign,
                    left: ast::PatOrExpr::Pat(P(ast::Pat::Ident(ast::Ident {
                        span: temp_span(),
                        sym: existing_name,
                        type_ann: None,
                        optional: false,
                    }))),
                    right: P(expr),
                }))),
                None => {
                    let name = scope.declare_mutable(target_ref);
                    ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
                        span: temp_span(),
                        kind: ast::VarDeclKind::Var,
                        decls: vec![ast::VarDeclarator {
                            span: temp_span(),
                            name: ast::Pat::Ident(ast::Ident {
                                span: temp_span(),
                                sym: name,
                                type_ann: None,
                                optional: false,
                            }),
                            init: Some(P(expr)),
                            definite: false,
                        }],
                        declare: false,
                    }))
                }
            }
        }
        ir::Stmt::WriteGlobal(target_name, source_ref) => unimplemented!(),
        ir::Stmt::WriteMember(obj_ref, prop_ref, source_ref) => unimplemented!(),
        ir::Stmt::Return(ref_) => unimplemented!(),
        ir::Stmt::Throw(ref_) => unimplemented!(),
        ir::Stmt::Break => unimplemented!(),
        ir::Stmt::Continue => unimplemented!(),
        ir::Stmt::Debugger => unimplemented!(),
        ir::Stmt::Block(block) => unimplemented!(),
        ir::Stmt::Loop(block) => unimplemented!(),
        ir::Stmt::For(kind, loop_var, source_ref, block) => unimplemented!(),
        ir::Stmt::IfElse(cond_ref, cons_block, alt_block) => unimplemented!(),
        ir::Stmt::Try(block, catch, finally) => unimplemented!(),
    }
}

fn convert_expr(expr: ir::Expr, scope: &scope::Ir) -> ast::Expr {
    unimplemented!()
}

fn ssa_to_expr(ssa_ref: ir::Ref<ir::SSA>, scope: &scope::Ir) -> ast::Expr {
    let name = match scope.get_ssa(&ssa_ref) {
        Some(name) => name,
        None => unreachable!("reading from undeclared SSA ref"),
    };
    ast::Expr::Ident(ast::Ident {
        span: temp_span(),
        sym: name,
        type_ann: None,
        optional: false,
    })
}
