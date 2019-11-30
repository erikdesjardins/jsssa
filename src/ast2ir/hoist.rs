use swc_atoms::JsWord;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;

pub enum Hoist {
    Everything,
    OnlyLetConst,
}

#[inline(never)] // for better profiling
pub fn hoist_fn_decls(block: &mut [ast::Stmt]) {
    // manually hoist function declarations at the toplevel/fn scopes
    block.sort_by_key(|stmt| match stmt {
        ast::Stmt::Decl(ast::Decl::Fn(_)) => 0,
        ast::Stmt::Labeled(ast::LabeledStmt { body, .. }) => match body.as_ref() {
            ast::Stmt::Decl(ast::Decl::Fn(_)) => 0,
            _ => 1,
        },
        _ => 1,
    });
}

#[inline(never)] // for better profiling
pub fn hoist_vars(block: &[ast::Stmt], scope: &mut scope::Ast, hoist: Hoist) -> Vec<ir::Stmt> {
    hoist_block(block, scope, &hoist, Level::First)
}

enum Level {
    First,
    Below,
}

fn hoist_block(
    body: &[ast::Stmt],
    scope: &mut scope::Ast,
    hoist: &Hoist,
    level: Level,
) -> Vec<ir::Stmt> {
    match (hoist, &level) {
        (Hoist::Everything, _) | (Hoist::OnlyLetConst, Level::First) => body
            .iter()
            .flat_map(|stmt| hoist_statement(stmt, scope, hoist, &level))
            .collect(),
        (Hoist::OnlyLetConst, Level::Below) => vec![],
    }
}

fn hoist_statement(
    stmt: &ast::Stmt,
    scope: &mut scope::Ast,
    hoist: &Hoist,
    level: &Level,
) -> Vec<ir::Stmt> {
    match stmt {
        ast::Stmt::Block(ast::BlockStmt { stmts, span: _ }) => {
            hoist_block(stmts, scope, hoist, Level::Below)
        }
        ast::Stmt::With(ast::WithStmt {
            obj: _,
            body,
            span: _,
        }) => hoist_statement(body, scope, hoist, &Level::Below),
        ast::Stmt::Labeled(ast::LabeledStmt {
            label: _,
            body,
            span: _,
        }) => hoist_statement(body, scope, hoist, level),
        ast::Stmt::If(ast::IfStmt {
            test: _,
            cons,
            alt,
            span: _,
        }) => {
            let mut decls = hoist_statement(cons, scope, hoist, &Level::Below);
            if let Some(alt) = alt {
                decls.extend(hoist_statement(alt, scope, hoist, &Level::Below));
            }
            decls
        }
        ast::Stmt::Switch(ast::SwitchStmt {
            discriminant: _,
            cases,
            span: _,
        }) => cases
            .iter()
            .flat_map(
                |ast::SwitchCase {
                     test: _,
                     cons,
                     span: _,
                 }| {
                    cons.iter()
                        .flat_map(|stmt| hoist_statement(stmt, scope, hoist, &Level::Below))
                        .collect::<Vec<_>>()
                },
            )
            .collect(),
        ast::Stmt::Try(ast::TryStmt {
            block: ast::BlockStmt { stmts, span: _ },
            handler,
            finalizer,
            span: _,
        }) => {
            let mut decls = hoist_block(stmts, scope, hoist, Level::Below);
            if let Some(ast::CatchClause {
                param: _,
                body: ast::BlockStmt { stmts, span: _ },
                span: _,
            }) = handler
            {
                decls.extend(hoist_block(stmts, scope, hoist, Level::Below));
            }
            if let Some(ast::BlockStmt { stmts, span: _ }) = finalizer {
                decls.extend(hoist_block(stmts, scope, hoist, Level::Below));
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
        }) => hoist_statement(body, scope, hoist, &Level::Below),
        ast::Stmt::For(ast::ForStmt {
            init,
            test: _,
            update: _,
            body,
            span: _,
        }) => {
            let mut decls = match init {
                Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
                    hoist_variable_declaration(var_decl, scope, hoist, &Level::Below)
                }
                Some(ast::VarDeclOrExpr::Expr(_)) | None => vec![],
            };
            decls.extend(hoist_statement(body, scope, hoist, &Level::Below));
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
                ast::VarDeclOrPat::VarDecl(var_decl) => {
                    hoist_variable_declaration(var_decl, scope, hoist, &Level::Below)
                }
                // bare patterns can't introduce variables
                ast::VarDeclOrPat::Pat(_) => vec![],
            };
            decls.extend(hoist_statement(body, scope, hoist, &Level::Below));
            decls
        }
        ast::Stmt::Decl(decl) => match decl {
            ast::Decl::Fn(fn_decl) => hoist_function_declaration(fn_decl, scope, hoist),
            ast::Decl::Var(var_decl) => hoist_variable_declaration(var_decl, scope, hoist, level),
            ast::Decl::Class(_) => unimplemented!("classes not supported"),
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

fn hoist_function_declaration(
    fn_decl: &ast::FnDecl,
    scope: &mut scope::Ast,
    hoist: &Hoist,
) -> Vec<ir::Stmt> {
    let ast::FnDecl {
        ident:
            ast::Ident {
                sym,
                span: _,
                type_ann: _,
                optional: _,
            },
        function: _,
        declare: _,
    } = fn_decl;

    match hoist {
        Hoist::Everything => {
            let fn_ref = scope.declare_mutable(sym.clone());
            let init_ref = ir::Ref::new("_ini");
            vec![
                ir::Stmt::Expr {
                    target: init_ref.clone(),
                    expr: ir::Expr::Undefined,
                },
                ir::Stmt::DeclareMutable {
                    target: fn_ref,
                    val: init_ref,
                },
            ]
        }
        Hoist::OnlyLetConst => vec![],
    }
}

fn hoist_variable_declaration(
    var_decl: &ast::VarDecl,
    scope: &mut scope::Ast,
    hoist: &Hoist,
    level: &Level,
) -> Vec<ir::Stmt> {
    let ast::VarDecl {
        kind,
        decls,
        span: _,
        declare: _,
    } = var_decl;
    match (hoist, level, kind) {
        (Hoist::Everything, _, ast::VarDeclKind::Var) => decls
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
        (_, Level::First, ast::VarDeclKind::Let) | (_, Level::First, ast::VarDeclKind::Const) => {
            decls.iter().for_each(
                |ast::VarDeclarator {
                     name,
                     init: _,
                     span: _,
                     definite: _,
                 }| {
                    let name = pat_to_ident(name);
                    // declare to reserve name, in case some function above the declaration uses it
                    scope.declare_mutable(name.clone());
                },
            );
            vec![]
        }
        (Hoist::OnlyLetConst, _, ast::VarDeclKind::Var)
        | (_, Level::Below, ast::VarDeclKind::Let)
        | (_, Level::Below, ast::VarDeclKind::Const) => vec![],
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
        | ast::Pat::Expr(_) => unimplemented!("complex patterns not supported"),
        ast::Pat::Invalid(_) => unreachable!(),
    }
}
