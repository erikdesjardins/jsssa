use swc_atoms::JsWord;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;

pub enum ShouldHoist {
    Yes,
    No,
}

#[inline(never)] // for better profiling
pub fn hoist(block: &[ast::Stmt], scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    hoist_block(block, scope)
}

fn hoist_block(body: &[ast::Stmt], scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    body.iter()
        .flat_map(|stmt| hoist_statement(stmt, scope))
        .collect()
}

fn hoist_statement(stmt: &ast::Stmt, scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    match stmt {
        ast::Stmt::Block(ast::BlockStmt { stmts, span: _ }) => hoist_block(stmts, scope),
        ast::Stmt::With(ast::WithStmt {
            obj: _,
            body,
            span: _,
        }) => hoist_statement(body, scope),
        ast::Stmt::Labeled(ast::LabeledStmt {
            label: _,
            body,
            span: _,
        }) => hoist_statement(body, scope),
        ast::Stmt::If(ast::IfStmt {
            test: _,
            cons,
            alt,
            span: _,
        }) => {
            let mut decls = hoist_statement(cons, scope);
            if let Some(alt) = alt {
                decls.extend(hoist_statement(alt, scope));
            }
            decls
        }
        ast::Stmt::Switch(_) => {
            // remember, switch cases aren't evaluated (!) until they're checked for equality
            unimplemented!("switch statements not yet supported")
        }
        ast::Stmt::Try(ast::TryStmt {
            block: ast::BlockStmt { stmts, span: _ },
            handler,
            finalizer,
            span: _,
        }) => {
            let mut decls = hoist_block(stmts, scope);
            if let Some(ast::CatchClause {
                param: _,
                body: ast::BlockStmt { stmts, span: _ },
                span: _,
            }) = handler
            {
                decls.extend(hoist_block(stmts, scope));
            }
            if let Some(ast::BlockStmt { stmts, span: _ }) = finalizer {
                decls.extend(hoist_block(stmts, scope));
            }
            decls
        }
        ast::Stmt::While(ast::WhileStmt {
            test: _,
            body,
            span: _,
        })
        | ast::Stmt::DoWhile(ast::DoWhileStmt {
            test: _,
            body,
            span: _,
        }) => hoist_statement(body, scope),
        ast::Stmt::For(ast::ForStmt {
            init,
            test: _,
            update: _,
            body,
            span: _,
        }) => {
            let mut decls = match init {
                Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
                    hoist_variable_declaration(var_decl, scope)
                }
                Some(ast::VarDeclOrExpr::Expr(_)) | None => vec![],
            };
            decls.extend(hoist_statement(body, scope));
            decls
        }
        ast::Stmt::ForIn(ast::ForInStmt {
            left,
            right: _,
            body,
            span: _,
        })
        | ast::Stmt::ForOf(ast::ForOfStmt {
            left,
            right: _,
            body,
            await_token: _,
            span: _,
        }) => {
            let mut decls = match left {
                ast::VarDeclOrPat::VarDecl(var_decl) => hoist_variable_declaration(var_decl, scope),
                // bare patterns can't introduce variables
                ast::VarDeclOrPat::Pat(_) => vec![],
            };
            decls.extend(hoist_statement(body, scope));
            decls
        }
        ast::Stmt::Decl(decl) => match decl {
            ast::Decl::Fn(_) => vec![],
            ast::Decl::Var(var_decl) => hoist_variable_declaration(var_decl, scope),
            ast::Decl::Class(_) => unimplemented!("classes not yet supported"),
            ast::Decl::TsInterface(_)
            | ast::Decl::TsTypeAlias(_)
            | ast::Decl::TsEnum(_)
            | ast::Decl::TsModule(_) => unreachable!(),
        },
        ast::Stmt::Expr(_)
        | ast::Stmt::Empty(_)
        | ast::Stmt::Debugger(_)
        | ast::Stmt::Return(_)
        | ast::Stmt::Break(_)
        | ast::Stmt::Continue(_)
        | ast::Stmt::Throw(_) => vec![],
    }
}

fn hoist_variable_declaration(var_decl: &ast::VarDecl, scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    let ast::VarDecl {
        kind,
        decls,
        span: _,
        declare: _,
    } = var_decl;
    match kind {
        ast::VarDeclKind::Var => decls
            .iter()
            .flat_map(
                |ast::VarDeclarator {
                     name,
                     init: _,
                     span: _,
                     definite: _,
                 }| {
                    let name = pat_to_ident(name);
                    match scope.get_mutable_in_current(name) {
                        None => {
                            let var_ref = scope.declare_mutable(name.clone());
                            let init_ref = ir::Ref::new("_ini");
                            vec![
                                ir::Stmt::Expr {
                                    target: init_ref.clone(),
                                    expr: ir::Expr::Undefined,
                                },
                                ir::Stmt::DeclareMutable {
                                    target: var_ref,
                                    val: init_ref,
                                },
                            ]
                        }
                        Some(_) => vec![],
                    }
                },
            )
            .collect(),
        ast::VarDeclKind::Let | ast::VarDeclKind::Const => vec![],
    }
}

fn pat_to_ident(pat: &ast::Pat) -> &JsWord {
    match pat {
        ast::Pat::Ident(ast::Ident {
            sym,
            span: _,
            type_ann: _,
            optional: _,
        }) => sym,
        ast::Pat::Array(_)
        | ast::Pat::Object(_)
        | ast::Pat::Rest(_)
        | ast::Pat::Assign(_)
        | ast::Pat::Expr(_) => unimplemented!("complex patterns not yet supported"),
    }
}
