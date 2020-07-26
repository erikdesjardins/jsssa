use std::iter;

use swc_atoms::JsWord;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::collections::Either;
use crate::ir;
use crate::ir::scope;
use crate::swc_globals;

use self::hoist::Hoist;

mod hoist;

#[inline(never)] // for better profiling
pub fn convert(_: &swc_globals::Initialized, ast: ast::Program) -> ir::Block {
    let body = match ast {
        ast::Program::Script(ast::Script {
            shebang: _,
            body,
            span: _,
        }) => body,
        ast::Program::Module(ast::Module {
            shebang: _,
            body,
            span: _,
        }) => body
            .into_iter()
            .map(|item| match item {
                ast::ModuleItem::Stmt(stmt) => stmt,
                ast::ModuleItem::ModuleDecl(_) => unimplemented!("module items not supported"),
            })
            .collect(),
    };

    convert_block(body, &scope::Ast::default(), Hoist::Everything)
}

fn convert_block(mut body: Vec<ast::Stmt>, parent_scope: &scope::Ast, hoist: Hoist) -> ir::Block {
    let mut scope = parent_scope.nested();

    match &hoist {
        Hoist::Everything => hoist::hoist_fn_decls(&mut body),
        Hoist::OnlyLetConst => {}
    };
    let mut stmts = hoist::hoist_vars(&body, &mut scope, hoist);

    let body_stmts = body
        .into_iter()
        .flat_map(|stmt| convert_statement(stmt, &mut scope));

    stmts.extend(body_stmts);

    ir::Block(stmts)
}

fn convert_statement(stmt: ast::Stmt, scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    match stmt {
        ast::Stmt::Expr(ast::ExprStmt { expr, span: _ }) => {
            let (mut stmts, last_expr) = convert_expression(*expr, scope);
            stmts.push(ir::Stmt::Expr {
                target: ir::Ref::dead(),
                expr: last_expr,
            });
            stmts
        }
        ast::Stmt::Block(ast::BlockStmt { stmts, span: _ }) => {
            let ir::Block(children) = convert_block(stmts, scope, Hoist::OnlyLetConst);
            children
        }
        ast::Stmt::Empty(ast::EmptyStmt { span: _ }) => vec![],
        ast::Stmt::Debugger(ast::DebuggerStmt { span: _ }) => vec![ir::Stmt::Debugger],
        ast::Stmt::With(_) => unimplemented!("with() statement not supported"),
        ast::Stmt::Return(ast::ReturnStmt { arg, span: _ }) => {
            let ref_ = ir::Ref::new("_ret");
            match arg {
                Some(arg) => {
                    let (mut stmts, return_value) = convert_expression(*arg, scope);
                    stmts.push(ir::Stmt::Expr {
                        target: ref_.clone(),
                        expr: return_value,
                    });
                    stmts.push(ir::Stmt::Return { val: ref_ });
                    stmts
                }
                None => vec![
                    ir::Stmt::Expr {
                        target: ref_.clone(),
                        expr: ir::Expr::Undefined,
                    },
                    ir::Stmt::Return { val: ref_ },
                ],
            }
        }
        ast::Stmt::Labeled(ast::LabeledStmt {
            label:
                ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                },
            body,
            span: _,
        }) => {
            let mut label_scope = scope.nested();
            let label_ref = label_scope.declare_label(sym);
            let body = convert_statement(*body, &mut label_scope);
            vec![ir::Stmt::Label {
                label: label_ref,
                body: ir::Block(body),
            }]
        }
        ast::Stmt::Break(ast::BreakStmt { label, span: _ }) => vec![ir::Stmt::Break {
            label: label.map(
                |ast::Ident {
                     sym,
                     span: _,
                     type_ann: _,
                     optional: _,
                 }| match scope.get_label(&sym) {
                    Some(label_ref) => label_ref.clone(),
                    None => unreachable!("breaking from undeclared label: {:?}", sym),
                },
            ),
        }],
        ast::Stmt::Continue(ast::ContinueStmt { label, span: _ }) => vec![ir::Stmt::Continue {
            label: label.map(
                |ast::Ident {
                     sym,
                     span: _,
                     type_ann: _,
                     optional: _,
                 }| match scope.get_label(&sym) {
                    Some(label_ref) => label_ref.clone(),
                    None => unreachable!("continuing from undeclared label: {:?}", sym),
                },
            ),
        }],
        ast::Stmt::If(ast::IfStmt {
            test,
            cons,
            alt,
            span: _,
        }) => {
            let ref_ = ir::Ref::new("_iff");
            let (mut stmts, test_value) = convert_expression(*test, scope);
            stmts.push(ir::Stmt::Expr {
                target: ref_.clone(),
                expr: test_value,
            });
            stmts.push(ir::Stmt::IfElse {
                cond: ref_,
                cons: ir::Block(convert_statement(*cons, &mut scope.nested())),
                alt: match alt {
                    Some(alt) => ir::Block(convert_statement(*alt, &mut scope.nested())),
                    None => ir::Block(vec![]),
                },
            });
            stmts
        }
        ast::Stmt::Switch(ast::SwitchStmt {
            discriminant,
            cases,
            span: _,
        }) => {
            let discr_ref = ir::Ref::new("_swi");
            let (mut stmts, discr_value) = convert_expression(*discriminant, scope);
            stmts.push(ir::Stmt::Expr {
                target: discr_ref.clone(),
                expr: discr_value,
            });

            let parent_scope = scope;
            let mut switch_scope = parent_scope.nested();

            cases.iter().for_each(
                |ast::SwitchCase {
                     test: _,
                     cons,
                     span: _,
                 }| {
                    let hoist_stmts =
                        hoist::hoist_vars(cons, &mut switch_scope, Hoist::OnlyLetConst);
                    assert!(hoist_stmts.is_empty(), "vars shouldn't be hoisted");
                },
            );

            let body_stmts = cases
                .into_iter()
                .flat_map(
                    |ast::SwitchCase {
                         test,
                         cons,
                         span: _,
                     }| {
                        let case_ref = match test {
                            Some(test) => {
                                let test = *test;
                                let safe_test = match test {
                                    ast::Expr::Lit(_) | ast::Expr::Ident(_) => test,
                                    _ => {
                                        // switch cases aren't evaluated (!) until they're checked for equality,
                                        // which is hard to model correctly without making IR much more complex
                                        unimplemented!("switch case with side effects: {:?}", test)
                                    }
                                };
                                let test_ref = ir::Ref::new("_tst");
                                let (test_stmts, test_value) =
                                    convert_expression(safe_test, parent_scope);
                                stmts.extend(test_stmts);
                                stmts.push(ir::Stmt::Expr {
                                    target: test_ref.clone(),
                                    expr: test_value,
                                });
                                Some(test_ref)
                            }
                            None => None,
                        };
                        iter::once(ir::Stmt::SwitchCase { val: case_ref })
                            .chain(
                                cons.into_iter()
                                    .flat_map(|stmt| convert_statement(stmt, &mut switch_scope)),
                            )
                            .collect::<Vec<_>>()
                    },
                )
                .collect();
            stmts.push(ir::Stmt::Switch {
                discr: discr_ref,
                body: ir::Block(body_stmts),
            });
            stmts
        }
        ast::Stmt::Throw(ast::ThrowStmt { arg, span: _ }) => {
            let ref_ = ir::Ref::new("_thr");
            let (mut stmts, throw_value) = convert_expression(*arg, scope);
            stmts.push(ir::Stmt::Expr {
                target: ref_.clone(),
                expr: throw_value,
            });
            stmts.push(ir::Stmt::Throw { val: ref_ });
            stmts
        }
        ast::Stmt::Try(ast::TryStmt {
            block,
            handler,
            finalizer,
            span: _,
        }) => {
            let try_ = ir::Stmt::Try {
                body: {
                    let ast::BlockStmt { stmts, span: _ } = block;
                    convert_block(stmts, scope, Hoist::OnlyLetConst)
                },
                catch: match handler {
                    Some(ast::CatchClause {
                        param,
                        body: ast::BlockStmt { stmts, span: _ },
                        span: _,
                    }) => {
                        let mut catch_scope = scope.nested();
                        let args = match param {
                            Some(param) => {
                                let name = pat_to_ident(param);
                                let catch_ref = catch_scope.declare_mutable(name);
                                let arg_ref = ir::Ref::new("_cat");
                                vec![
                                    ir::Stmt::Expr {
                                        target: arg_ref.clone(),
                                        expr: ir::Expr::Argument { index: 0 },
                                    },
                                    ir::Stmt::DeclareMutable {
                                        target: catch_ref,
                                        val: arg_ref,
                                    },
                                ]
                            }
                            None => vec![],
                        };
                        let ir::Block(children) =
                            convert_block(stmts, &catch_scope, Hoist::OnlyLetConst);
                        let body = args.into_iter().chain(children).collect();
                        ir::Block(body)
                    }
                    None => ir::Block(vec![]),
                },
                finally: Box::new(match finalizer {
                    Some(ast::BlockStmt { stmts, span: _ }) => {
                        convert_block(stmts, scope, Hoist::OnlyLetConst)
                    }
                    None => ir::Block(vec![]),
                }),
            };
            vec![try_]
        }
        while_stmt @ ast::Stmt::While(_) | while_stmt @ ast::Stmt::DoWhile(_) => {
            let (test, body, prefix) = match while_stmt {
                ast::Stmt::While(ast::WhileStmt {
                    test,
                    body,
                    span: _,
                }) => (test, body, true),
                ast::Stmt::DoWhile(ast::DoWhileStmt {
                    body,
                    test,
                    span: _,
                }) => (test, body, false),
                _ => unreachable!(),
            };
            let cond_ref = ir::Ref::new("_whl");
            let (mut test_stmts, test_value) = convert_expression(*test, scope);
            test_stmts.push(ir::Stmt::Expr {
                target: cond_ref.clone(),
                expr: test_value,
            });
            test_stmts.push(ir::Stmt::IfElse {
                cond: cond_ref,
                cons: ir::Block(vec![]),
                alt: ir::Block(vec![ir::Stmt::Break { label: None }]),
            });
            let body_stmts = convert_statement(*body, &mut scope.nested());
            let stmts = if prefix {
                test_stmts.into_iter().chain(body_stmts)
            } else {
                body_stmts.into_iter().chain(test_stmts)
            }
            .collect();
            vec![ir::Stmt::Loop {
                body: ir::Block(stmts),
            }]
        }
        ast::Stmt::For(ast::ForStmt {
            init,
            test,
            update,
            body,
            span: _,
        }) => {
            let mut stmts = match init {
                Some(ast::VarDeclOrExpr::Expr(init_expr)) => {
                    let (mut init_stmts, init_value) = convert_expression(*init_expr, scope);
                    init_stmts.push(ir::Stmt::Expr {
                        target: ir::Ref::dead(),
                        expr: init_value,
                    });
                    init_stmts
                }
                Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
                    convert_variable_declaration(var_decl, scope)
                }
                None => vec![],
            };
            stmts.push(ir::Stmt::Loop {
                body: ir::Block({
                    let mut stmts = match test {
                        Some(test) => {
                            let cond_ref = ir::Ref::new("_for");
                            let (mut test_stmts, test_value) = convert_expression(*test, scope);
                            test_stmts.push(ir::Stmt::Expr {
                                target: cond_ref.clone(),
                                expr: test_value,
                            });
                            test_stmts.push(ir::Stmt::IfElse {
                                cond: cond_ref,
                                cons: ir::Block(vec![]),
                                alt: ir::Block(vec![ir::Stmt::Break { label: None }]),
                            });
                            test_stmts
                        }
                        None => vec![],
                    };
                    stmts.extend(convert_statement(*body, &mut scope.nested()));
                    if let Some(update) = update {
                        let (update_stmts, update_value) = convert_expression(*update, scope);
                        stmts.extend(update_stmts);
                        stmts.push(ir::Stmt::Expr {
                            target: ir::Ref::dead(),
                            expr: update_value,
                        });
                    }
                    stmts
                }),
            });
            stmts
        }
        for_stmt @ ast::Stmt::ForIn(_) | for_stmt @ ast::Stmt::ForOf(_) => {
            let (kind, left, right, body) = match for_stmt {
                ast::Stmt::ForIn(ast::ForInStmt {
                    left,
                    right,
                    body,
                    span: _,
                }) => (ir::ForKind::In, left, right, body),
                ast::Stmt::ForOf(ast::ForOfStmt {
                    left,
                    right,
                    body,
                    await_token,
                    span: _,
                }) => {
                    assert!(await_token.is_none(), "for-await-of not supported");
                    (ir::ForKind::Of, left, right, body)
                }
                _ => unreachable!(),
            };
            let mut stmts = vec![];
            let (ele_kind, ele_name) = match left {
                ast::VarDeclOrPat::VarDecl(ast::VarDecl {
                    kind,
                    decls,
                    span: _,
                    declare: _,
                }) => {
                    let mut decls_iter = decls.into_iter();
                    match (decls_iter.next(), decls_iter.next()) {
                        (
                            Some(ast::VarDeclarator {
                                name,
                                init,
                                span: _,
                                definite: _,
                            }),
                            None,
                        ) => {
                            if let Some(init) = init {
                                // for in/of var decls can have an initializer, which is pointless
                                let (init_stmts, init_value) = convert_expression(*init, scope);
                                stmts.extend(init_stmts);
                                stmts.push(ir::Stmt::Expr {
                                    target: ir::Ref::dead(),
                                    expr: init_value,
                                });
                            }
                            let name = pat_to_ident(name);
                            (Some(kind), name)
                        }
                        (Some(_), Some(_)) | (None, _) => {
                            unreachable!("for in/of binding does not have exactly 1 declaration")
                        }
                    }
                }
                ast::VarDeclOrPat::Pat(pat) => {
                    let name = pat_to_ident(pat);
                    (None, name)
                }
            };
            let right_ref = ir::Ref::new("_rhs");
            let (right_stmts, right_value) = convert_expression(*right, scope);
            stmts.extend(right_stmts);
            stmts.push(ir::Stmt::Expr {
                target: right_ref.clone(),
                expr: right_value,
            });
            let mut for_scope = scope.nested();
            let arg_ref = ir::Ref::new("_for");
            let args = vec![
                ir::Stmt::Expr {
                    target: arg_ref.clone(),
                    expr: ir::Expr::Argument { index: 0 },
                },
                match ele_kind {
                    None => match for_scope.get_mutable(&ele_name) {
                        Some(binding) => ir::Stmt::WriteMutable {
                            target: binding.clone(),
                            val: arg_ref,
                        },
                        None => ir::Stmt::WriteGlobal {
                            target: ele_name,
                            val: arg_ref,
                        },
                    },
                    Some(ast::VarDeclKind::Var) => match for_scope.get_mutable(&ele_name) {
                        Some(binding) => ir::Stmt::WriteMutable {
                            target: binding.clone(),
                            val: arg_ref,
                        },
                        None => unreachable!("foreach var not hoisted: {:?}", ele_name),
                    },
                    Some(ast::VarDeclKind::Let) | Some(ast::VarDeclKind::Const) => {
                        let ele_ref = for_scope.declare_mutable(ele_name);
                        ir::Stmt::DeclareMutable {
                            target: ele_ref,
                            val: arg_ref,
                        }
                    }
                },
            ];
            let children = convert_statement(*body, &mut for_scope);
            let body = args.into_iter().chain(children).collect();
            stmts.push(ir::Stmt::ForEach {
                kind,
                init: right_ref,
                body: ir::Block(body),
            });
            stmts
        }
        ast::Stmt::Decl(decl) => match decl {
            ast::Decl::Fn(ast::FnDecl {
                ident,
                function:
                    ast::Function {
                        params,
                        decorators: _,
                        body,
                        is_generator,
                        is_async,
                        span: _,
                        type_params: _,
                        return_type: _,
                    },
                declare: _,
            }) => {
                let ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                } = ident;
                let mut fn_scope = scope.nested();
                let recursive_ref = fn_scope.declare_mutable(sym.clone());
                let cur_fn_ref = ir::Ref::new(recursive_ref.name_hint());
                let cur_fn = vec![
                    ir::Stmt::Expr {
                        target: cur_fn_ref.clone(),
                        expr: ir::Expr::CurrentFunction,
                    },
                    ir::Stmt::DeclareMutable {
                        target: recursive_ref,
                        val: cur_fn_ref,
                    },
                ];
                let params = params
                    .into_iter()
                    .enumerate()
                    .flat_map(|(i, param)| {
                        let ast::Param {
                            pat,
                            decorators: _,
                            span: _,
                        } = param;
                        let name = pat_to_ident(pat);
                        let param_ref = ir::Ref::new(&name);
                        let param_expr = ir::Stmt::Expr {
                            target: param_ref.clone(),
                            expr: ir::Expr::Argument { index: i },
                        };
                        match fn_scope.get_mutable_in_current(&name) {
                            None => vec![
                                param_expr,
                                ir::Stmt::DeclareMutable {
                                    target: fn_scope.declare_mutable(name),
                                    val: param_ref,
                                },
                            ],
                            // recursive_ref already declared this var (arg shadows fn name)
                            Some(arg) => vec![
                                param_expr,
                                ir::Stmt::WriteMutable {
                                    target: arg.clone(),
                                    val: param_ref,
                                },
                            ],
                        }
                    })
                    .collect::<Vec<_>>();
                let body = match body {
                    Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                    None => unreachable!("bodyless function type declaration"),
                };
                let ir::Block(children) = convert_block(body, &fn_scope, Hoist::Everything);
                let block = cur_fn.into_iter().chain(params).chain(children).collect();
                let fn_ref = ir::Ref::new("_fun");
                let fn_binding = match scope.get_mutable(&sym) {
                    Some(fn_ref) => fn_ref.clone(),
                    None => unreachable!("fn not hoisted/predeclared: {:?}", sym),
                };
                vec![
                    ir::Stmt::Expr {
                        target: fn_ref.clone(),
                        expr: ir::Expr::Function {
                            kind: ir::FnKind::Func {
                                is_async,
                                is_generator,
                            },
                            body: ir::Block(block),
                        },
                    },
                    ir::Stmt::WriteMutable {
                        target: fn_binding,
                        val: fn_ref,
                    },
                ]
            }
            ast::Decl::Var(var_decl) => convert_variable_declaration(var_decl, scope),
            ast::Decl::Class(_) => unimplemented!("classes not supported"),
            ast::Decl::TsInterface(_)
            | ast::Decl::TsTypeAlias(_)
            | ast::Decl::TsEnum(_)
            | ast::Decl::TsModule(_) => unreachable!(),
        },
    }
}

fn convert_expression(expr: ast::Expr, scope: &scope::Ast) -> (Vec<ir::Stmt>, ir::Expr) {
    match expr {
        ast::Expr::Ident(ast::Ident {
            sym,
            span: _,
            type_ann: _,
            optional: _,
        }) => {
            let expr = match scope.get_mutable(&sym) {
                Some(ref_) => ir::Expr::ReadMutable {
                    source: ref_.clone(),
                },
                None => ir::Expr::ReadGlobal { source: sym },
            };
            (vec![], expr)
        }
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Regex(ast::Regex {
                exp,
                flags,
                span: _,
            }) => (vec![], ir::Expr::RegExp { regex: exp, flags }),
            ast::Lit::Null(ast::Null { span: _ }) => (vec![], ir::Expr::Null),
            ast::Lit::Str(ast::Str {
                value,
                has_escape: _,
                span: _,
            }) => (vec![], ir::Expr::String { value }),
            ast::Lit::Bool(ast::Bool { value, span: _ }) => (vec![], ir::Expr::Bool { value }),
            ast::Lit::Num(ast::Number { value, span: _ }) => (
                vec![],
                ir::Expr::Number {
                    value: ir::F64::from(value),
                },
            ),
            ast::Lit::BigInt(_) => unimplemented!("bigint not supported"),
            ast::Lit::JSXText(_) => unreachable!(),
        },
        ast::Expr::This(ast::ThisExpr { span: _ }) => (vec![], ir::Expr::This),
        ast::Expr::Yield(ast::YieldExpr {
            arg,
            delegate,
            span: _,
        }) => {
            let ref_ = ir::Ref::new("_yld");
            let (mut stmts, yield_value) = match arg {
                Some(argument) => convert_expression(*argument, scope),
                None => (vec![], ir::Expr::Undefined),
            };
            stmts.push(ir::Stmt::Expr {
                target: ref_.clone(),
                expr: yield_value,
            });
            let kind = if delegate {
                ir::YieldKind::Delegate
            } else {
                ir::YieldKind::Single
            };
            (stmts, ir::Expr::Yield { kind, val: ref_ })
        }
        ast::Expr::Arrow(ast::ArrowExpr {
            params,
            body,
            is_async,
            is_generator,
            span: _,
            type_params: _,
            return_type: _,
        }) => {
            let mut fn_scope = scope.nested();
            let params = params
                .into_iter()
                .enumerate()
                .flat_map(|(i, param)| {
                    let name = pat_to_ident(param);
                    let arg = fn_scope.declare_mutable(name);
                    let param_ref = ir::Ref::new(arg.name_hint());
                    vec![
                        ir::Stmt::Expr {
                            target: param_ref.clone(),
                            expr: ir::Expr::Argument { index: i },
                        },
                        ir::Stmt::DeclareMutable {
                            target: arg,
                            val: param_ref,
                        },
                    ]
                })
                .collect::<Vec<_>>();
            let ir::Block(children) = match body {
                ast::BlockStmtOrExpr::BlockStmt(block) => {
                    let ast::BlockStmt { stmts, span: _ } = block;
                    convert_block(stmts, &fn_scope, Hoist::Everything)
                }
                ast::BlockStmtOrExpr::Expr(expr) => {
                    let ref_ = ir::Ref::new("_arr");
                    let (mut stmts, return_value) = convert_expression(*expr, &fn_scope);
                    stmts.push(ir::Stmt::Expr {
                        target: ref_.clone(),
                        expr: return_value,
                    });
                    stmts.push(ir::Stmt::Return { val: ref_ });
                    ir::Block(stmts)
                }
            };
            let body = params.into_iter().chain(children).collect();
            assert!(!is_generator, "generator arrow function");
            let func = ir::Expr::Function {
                kind: ir::FnKind::Arrow { is_async },
                body: ir::Block(body),
            };
            (vec![], func)
        }
        ast::Expr::Await(ast::AwaitExpr { arg, span: _ }) => {
            let ref_ = ir::Ref::new("_awa");
            let (mut stmts, await_value) = convert_expression(*arg, scope);
            stmts.push(ir::Stmt::Expr {
                target: ref_.clone(),
                expr: await_value,
            });
            (stmts, ir::Expr::Await { val: ref_ })
        }
        ast::Expr::Array(ast::ArrayLit { elems, span: _ }) => {
            let mut statements = vec![];
            let elements = elems
                .into_iter()
                .map(|ele| {
                    ele.map(|ast::ExprOrSpread { spread, expr }| {
                        let kind = match spread {
                            None => ir::EleKind::Single,
                            Some(_) => ir::EleKind::Spread,
                        };
                        let ref_ = ir::Ref::new("_ele");
                        let (stmts, ele_value) = convert_expression(*expr, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Expr {
                            target: ref_.clone(),
                            expr: ele_value,
                        });
                        (kind, ref_)
                    })
                })
                .collect();
            (statements, ir::Expr::Array { elems: elements })
        }
        ast::Expr::Object(ast::ObjectLit { props, span: _ }) => {
            let mut statements = vec![];
            let properties = props
                .into_iter()
                .map(|prop| match prop {
                    ast::PropOrSpread::Prop(prop) => match *prop {
                        prop @ ast::Prop::Shorthand(_) | prop @ ast::Prop::KeyValue(_) => {
                            let (key, value) = match prop {
                                ast::Prop::Shorthand(ident) => {
                                    let ast::Ident {
                                        sym,
                                        span,
                                        type_ann: _,
                                        optional: _,
                                    } = ident.clone();
                                    let key = ast::Expr::Lit(ast::Lit::Str(ast::Str {
                                        span,
                                        value: sym,
                                        has_escape: false,
                                    }));
                                    (key, ast::Expr::Ident(ident))
                                }
                                ast::Prop::KeyValue(ast::KeyValueProp { key, value }) => {
                                    (propname_to_expr(key), *value)
                                }
                                _ => unreachable!(),
                            };
                            let ref_key = ir::Ref::new("_key");
                            let (stmts, key_value) = convert_expression(key, scope);
                            statements.extend(stmts);
                            statements.push(ir::Stmt::Expr {
                                target: ref_key.clone(),
                                expr: key_value,
                            });
                            let ref_value = ir::Ref::new("_val");
                            let (stmts, value_value) = convert_expression(value, scope);
                            statements.extend(stmts);
                            statements.push(ir::Stmt::Expr {
                                target: ref_value.clone(),
                                expr: value_value,
                            });
                            (ir::PropKind::Simple, ref_key, ref_value)
                        }
                        prop @ ast::Prop::Getter(_)
                        | prop @ ast::Prop::Setter(_)
                        | prop @ ast::Prop::Method(_) => {
                            let (kind, name, function) = match prop {
                                ast::Prop::Getter(ast::GetterProp {
                                    key,
                                    body,
                                    span,
                                    type_ann: _,
                                }) => (
                                    ir::PropKind::Get,
                                    key,
                                    ast::Function {
                                        params: vec![],
                                        decorators: vec![],
                                        span,
                                        body,
                                        is_generator: false,
                                        is_async: false,
                                        type_params: None,
                                        return_type: None,
                                    },
                                ),
                                ast::Prop::Setter(ast::SetterProp {
                                    key,
                                    param,
                                    body,
                                    span,
                                }) => (
                                    ir::PropKind::Set,
                                    key,
                                    ast::Function {
                                        params: vec![ast::Param {
                                            span: param.span(),
                                            pat: param,
                                            decorators: vec![],
                                        }],
                                        decorators: vec![],
                                        span,
                                        body,
                                        is_generator: false,
                                        is_async: false,
                                        type_params: None,
                                        return_type: None,
                                    },
                                ),
                                ast::Prop::Method(ast::MethodProp { key, function }) => {
                                    (ir::PropKind::Simple, key, function)
                                }
                                _ => unreachable!(),
                            };
                            let key = propname_to_expr(name);
                            let ast::Function {
                                params,
                                decorators: _,
                                body: block,
                                is_generator,
                                is_async,
                                span: _,
                                type_params: _,
                                return_type: _,
                            } = function;

                            let ref_key = ir::Ref::new("_key");
                            let (stmts, key_value) = convert_expression(key, scope);
                            statements.extend(stmts);
                            statements.push(ir::Stmt::Expr {
                                target: ref_key.clone(),
                                expr: key_value,
                            });

                            let mut fn_scope = scope.nested();
                            let params = params
                                .into_iter()
                                .enumerate()
                                .flat_map(|(i, param)| {
                                    let ast::Param {
                                        pat,
                                        decorators: _,
                                        span: _,
                                    } = param;
                                    let name = pat_to_ident(pat);
                                    let arg = fn_scope.declare_mutable(name);
                                    let param_ref = ir::Ref::new(arg.name_hint());
                                    vec![
                                        ir::Stmt::Expr {
                                            target: param_ref.clone(),
                                            expr: ir::Expr::Argument { index: i },
                                        },
                                        ir::Stmt::DeclareMutable {
                                            target: arg,
                                            val: param_ref,
                                        },
                                    ]
                                })
                                .collect::<Vec<_>>();
                            let body = match block {
                                Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                                None => unreachable!("object method/accessor without body"),
                            };
                            let ir::Block(children) =
                                convert_block(body, &fn_scope, Hoist::Everything);
                            let block = params.into_iter().chain(children).collect();
                            let fn_value = ir::Expr::Function {
                                kind: ir::FnKind::Func {
                                    is_async,
                                    is_generator,
                                },
                                body: ir::Block(block),
                            };
                            let ref_value = ir::Ref::new("_val");
                            statements.push(ir::Stmt::Expr {
                                target: ref_value.clone(),
                                expr: fn_value,
                            });

                            (kind, ref_key, ref_value)
                        }
                        ast::Prop::Assign(_) => unreachable!("assignment prop in object literal"),
                    },
                    ast::PropOrSpread::Spread(_) => unimplemented!("object spread not implemented"),
                })
                .collect();
            (statements, ir::Expr::Object { props: properties })
        }
        ast::Expr::Fn(ast::FnExpr {
            ident,
            function:
                ast::Function {
                    params,
                    decorators: _,
                    body,
                    is_generator,
                    is_async,
                    span: _,
                    type_params: _,
                    return_type: _,
                },
        }) => {
            let mut fn_scope = scope.nested();
            let cur_fn = match ident {
                Some(ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                }) => {
                    let recursive_ref = fn_scope.declare_mutable(sym);
                    let cur_fn_ref = ir::Ref::new(recursive_ref.name_hint());
                    vec![
                        ir::Stmt::Expr {
                            target: cur_fn_ref.clone(),
                            expr: ir::Expr::CurrentFunction,
                        },
                        ir::Stmt::DeclareMutable {
                            target: recursive_ref,
                            val: cur_fn_ref,
                        },
                    ]
                }
                None => vec![],
            };
            let params = params
                .into_iter()
                .enumerate()
                .flat_map(|(i, param)| {
                    let ast::Param {
                        pat,
                        decorators: _,
                        span: _,
                    } = param;
                    let name = pat_to_ident(pat);
                    let param_ref = ir::Ref::new(&name);
                    let param_expr = ir::Stmt::Expr {
                        target: param_ref.clone(),
                        expr: ir::Expr::Argument { index: i },
                    };
                    match fn_scope.get_mutable_in_current(&name) {
                        None => vec![
                            param_expr,
                            ir::Stmt::DeclareMutable {
                                target: fn_scope.declare_mutable(name),
                                val: param_ref,
                            },
                        ],
                        // recursive_ref already declared this var (arg shadows fn name)
                        Some(arg) => vec![
                            param_expr,
                            ir::Stmt::WriteMutable {
                                target: arg.clone(),
                                val: param_ref,
                            },
                        ],
                    }
                })
                .collect::<Vec<_>>();
            let body = match body {
                Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                None => unreachable!("bodyless function type declaration"),
            };
            let ir::Block(children) = convert_block(body, &fn_scope, Hoist::Everything);
            let block = cur_fn.into_iter().chain(params).chain(children).collect();
            let func = ir::Expr::Function {
                kind: ir::FnKind::Func {
                    is_async,
                    is_generator,
                },
                body: ir::Block(block),
            };
            (vec![], func)
        }
        ast::Expr::Unary(ast::UnaryExpr { op, arg, span: _ }) => {
            let op = match op {
                ast::UnaryOp::Plus => ir::UnaryOp::Plus,
                ast::UnaryOp::Minus => ir::UnaryOp::Minus,
                ast::UnaryOp::Bang => ir::UnaryOp::Not,
                ast::UnaryOp::Tilde => ir::UnaryOp::Tilde,
                ast::UnaryOp::TypeOf => ir::UnaryOp::Typeof,
                ast::UnaryOp::Void => ir::UnaryOp::Void,
                // need to preserve member access
                ast::UnaryOp::Delete => match *arg {
                    ast::Expr::Member(expr) => {
                        let ast::MemberExpr {
                            obj,
                            prop,
                            computed,
                            span: _,
                        } = expr;
                        let obj_ref = ir::Ref::new("_obj");
                        let (mut stmts, obj_value) = convert_expr_or_super(obj, scope);
                        stmts.push(ir::Stmt::Expr {
                            target: obj_ref.clone(),
                            expr: obj_value,
                        });
                        let prop_ref = ir::Ref::new("_prp");
                        let (prop_stmts, prop_value) = if computed {
                            convert_expression(*prop, scope)
                        } else {
                            match *prop {
                                ast::Expr::Ident(ast::Ident {
                                    sym,
                                    span: _,
                                    type_ann: _,
                                    optional: _,
                                }) => (vec![], ir::Expr::String { value: sym }),
                                e => unreachable!("non-computed property is not an ident: {:?}", e),
                            }
                        };
                        stmts.extend(prop_stmts);
                        stmts.push(ir::Stmt::Expr {
                            target: prop_ref.clone(),
                            expr: prop_value,
                        });
                        return (
                            stmts,
                            ir::Expr::Delete {
                                obj: obj_ref,
                                prop: prop_ref,
                            },
                        );
                    }
                    _ => unimplemented!("deletion of non-MemberExpression not supported"),
                },
            };
            let ref_ = ir::Ref::new("_una");
            let (mut stmts, expr_value) = convert_expression(*arg, scope);
            stmts.push(ir::Stmt::Expr {
                target: ref_.clone(),
                expr: expr_value,
            });
            (stmts, ir::Expr::Unary { op, val: ref_ })
        }
        ast::Expr::Update(ast::UpdateExpr {
            op,
            arg,
            prefix,
            span: _,
        }) => {
            let one_ref = ir::Ref::new("_one");
            let read_ref = ir::Ref::new("_rdr");
            let write_ref = ir::Ref::new("_wri");
            let (read, write) = match *arg {
                ast::Expr::Ident(ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                }) => match scope.get_mutable(&sym) {
                    Some(ref_) => (
                        ir::Expr::ReadMutable {
                            source: ref_.clone(),
                        },
                        ir::Stmt::WriteMutable {
                            target: ref_.clone(),
                            val: write_ref.clone(),
                        },
                    ),
                    None => (
                        ir::Expr::ReadGlobal {
                            source: sym.clone(),
                        },
                        ir::Stmt::WriteGlobal {
                            target: sym,
                            val: write_ref.clone(),
                        },
                    ),
                },
                arg => unimplemented!("unexpected UpdateExpression argument: {:?}", arg),
            };
            let op = match op {
                ast::UpdateOp::PlusPlus => ir::BinaryOp::Add,
                ast::UpdateOp::MinusMinus => ir::BinaryOp::Sub,
            };
            let stmts = vec![
                ir::Stmt::Expr {
                    target: read_ref.clone(),
                    expr: read,
                },
                ir::Stmt::Expr {
                    target: one_ref.clone(),
                    expr: ir::Expr::Number {
                        value: ir::F64::from(1.0),
                    },
                },
                ir::Stmt::Expr {
                    target: write_ref.clone(),
                    expr: ir::Expr::Binary {
                        op,
                        left: read_ref.clone(),
                        right: one_ref,
                    },
                },
                write,
            ];
            let value = if prefix { write_ref } else { read_ref };
            (stmts, ir::Expr::Read { source: value })
        }
        ast::Expr::Bin(ast::BinExpr {
            op,
            left,
            right,
            span: _,
        }) => {
            match op {
                // technically should be LogicalOp
                op @ ast::BinaryOp::LogicalOr | op @ ast::BinaryOp::LogicalAnd => {
                    let left_ref = ir::Ref::new("_prd");
                    let value_ref = ir::Ref::new("_log");
                    let (mut stmts, left_value) = convert_expression(*left, scope);
                    stmts.push(ir::Stmt::Expr {
                        target: left_ref.clone(),
                        expr: left_value,
                    });
                    stmts.push(ir::Stmt::DeclareMutable {
                        target: value_ref.clone(),
                        val: left_ref.clone(),
                    });
                    let (consequent, alternate) = {
                        let right_ref = ir::Ref::new("_cns");
                        let (mut right_stmts, right_value) = convert_expression(*right, scope);
                        right_stmts.push(ir::Stmt::Expr {
                            target: right_ref.clone(),
                            expr: right_value,
                        });
                        right_stmts.push(ir::Stmt::WriteMutable {
                            target: value_ref.clone(),
                            val: right_ref,
                        });
                        let full = ir::Block(right_stmts);
                        let empty = ir::Block(vec![]);
                        match op {
                            ast::BinaryOp::LogicalOr => (empty, full),
                            ast::BinaryOp::LogicalAnd => (full, empty),
                            _ => unreachable!(),
                        }
                    };
                    stmts.push(ir::Stmt::IfElse {
                        cond: left_ref,
                        cons: consequent,
                        alt: alternate,
                    });
                    (stmts, ir::Expr::ReadMutable { source: value_ref })
                }
                op => {
                    let op = match op {
                        ast::BinaryOp::EqEq => ir::BinaryOp::EqEq,
                        ast::BinaryOp::NotEq => ir::BinaryOp::NotEq,
                        ast::BinaryOp::EqEqEq => ir::BinaryOp::StrictEq,
                        ast::BinaryOp::NotEqEq => ir::BinaryOp::NotStrictEq,
                        ast::BinaryOp::Lt => ir::BinaryOp::Lt,
                        ast::BinaryOp::LtEq => ir::BinaryOp::LtEq,
                        ast::BinaryOp::Gt => ir::BinaryOp::Gt,
                        ast::BinaryOp::GtEq => ir::BinaryOp::GtEq,
                        ast::BinaryOp::LShift => ir::BinaryOp::ShiftLeft,
                        ast::BinaryOp::RShift => ir::BinaryOp::ShiftRight,
                        ast::BinaryOp::ZeroFillRShift => ir::BinaryOp::ShiftRightZero,
                        ast::BinaryOp::Add => ir::BinaryOp::Add,
                        ast::BinaryOp::Sub => ir::BinaryOp::Sub,
                        ast::BinaryOp::Mul => ir::BinaryOp::Mul,
                        ast::BinaryOp::Div => ir::BinaryOp::Div,
                        ast::BinaryOp::Mod => ir::BinaryOp::Mod,
                        ast::BinaryOp::BitOr => ir::BinaryOp::BitOr,
                        ast::BinaryOp::BitXor => ir::BinaryOp::BitXor,
                        ast::BinaryOp::BitAnd => ir::BinaryOp::BitAnd,
                        ast::BinaryOp::Exp => ir::BinaryOp::Exp,
                        ast::BinaryOp::In => ir::BinaryOp::In,
                        ast::BinaryOp::InstanceOf => ir::BinaryOp::Instanceof,
                        ast::BinaryOp::NullishCoalescing => {
                            unimplemented!("nullish coalescing not supported")
                        }
                        ast::BinaryOp::LogicalOr | ast::BinaryOp::LogicalAnd => unreachable!(),
                    };
                    let left_ref = ir::Ref::new("_lhs");
                    let (mut stmts, left_value) = convert_expression(*left, scope);
                    stmts.push(ir::Stmt::Expr {
                        target: left_ref.clone(),
                        expr: left_value,
                    });
                    let right_ref = ir::Ref::new("_rhs");
                    let (right_stmts, right_value) = convert_expression(*right, scope);
                    stmts.extend(right_stmts);
                    stmts.push(ir::Stmt::Expr {
                        target: right_ref.clone(),
                        expr: right_value,
                    });
                    (
                        stmts,
                        ir::Expr::Binary {
                            op,
                            left: left_ref,
                            right: right_ref,
                        },
                    )
                }
            }
        }
        ast::Expr::Assign(ast::AssignExpr {
            op,
            left,
            right,
            span: _,
        }) => {
            let ident_or_member = match left {
                ast::PatOrExpr::Pat(pat) => match *pat {
                    ast::Pat::Ident(ident) => Either::A(ident),
                    ast::Pat::Expr(expr) => match *expr {
                        ast::Expr::Ident(ident) => Either::A(ident),
                        ast::Expr::Member(member) => Either::B(member),
                        e => unimplemented!("assigning to non member-expression pattern: {:?}", e),
                    },
                    p => unimplemented!("assigning to complex pattern: {:?}", p),
                },
                ast::PatOrExpr::Expr(expr) => match *expr {
                    ast::Expr::Ident(ident) => Either::A(ident),
                    ast::Expr::Member(member) => Either::B(member),
                    e => unreachable!("assigning to unsupported expression: {:?}", e),
                },
            };
            let value_ref = ir::Ref::new("_val");
            let (mut stmts, read_expr, write_stmt) = match ident_or_member {
                Either::A(ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                }) => match scope.get_mutable(&sym) {
                    Some(binding) => (
                        vec![],
                        ir::Expr::ReadMutable {
                            source: binding.clone(),
                        },
                        ir::Stmt::WriteMutable {
                            target: binding.clone(),
                            val: value_ref.clone(),
                        },
                    ),
                    None => (
                        vec![],
                        ir::Expr::ReadGlobal {
                            source: sym.clone(),
                        },
                        ir::Stmt::WriteGlobal {
                            target: sym.clone(),
                            val: value_ref.clone(),
                        },
                    ),
                },
                Either::B(ast::MemberExpr {
                    obj,
                    prop,
                    computed,
                    span: _,
                }) => {
                    let obj_ref = ir::Ref::new("_obj");
                    let prop_ref = ir::Ref::new("_prp");
                    let (mut stmts, obj_value) = convert_expr_or_super(obj, scope);
                    stmts.push(ir::Stmt::Expr {
                        target: obj_ref.clone(),
                        expr: obj_value,
                    });
                    let (prop_stmts, prop_value) = if computed {
                        convert_expression(*prop, scope)
                    } else {
                        match *prop {
                            ast::Expr::Ident(ast::Ident {
                                sym,
                                span: _,
                                type_ann: _,
                                optional: _,
                            }) => (vec![], ir::Expr::String { value: sym }),
                            e => unreachable!("non-computed property is not an ident: {:?}", e),
                        }
                    };
                    stmts.extend(prop_stmts);
                    stmts.push(ir::Stmt::Expr {
                        target: prop_ref.clone(),
                        expr: prop_value,
                    });
                    (
                        stmts,
                        ir::Expr::ReadMember {
                            obj: obj_ref.clone(),
                            prop: prop_ref.clone(),
                        },
                        ir::Stmt::WriteMember {
                            obj: obj_ref,
                            prop: prop_ref,
                            val: value_ref.clone(),
                        },
                    )
                }
            };
            match op {
                ast::AssignOp::Assign => {
                    let (right_stmts, right_val) = convert_expression(*right, scope);
                    stmts.extend(right_stmts);
                    stmts.push(ir::Stmt::Expr {
                        target: value_ref.clone(),
                        expr: right_val,
                    });
                    stmts.push(write_stmt);
                }
                op @ ast::AssignOp::AddAssign
                | op @ ast::AssignOp::SubAssign
                | op @ ast::AssignOp::MulAssign
                | op @ ast::AssignOp::DivAssign
                | op @ ast::AssignOp::ModAssign
                | op @ ast::AssignOp::LShiftAssign
                | op @ ast::AssignOp::RShiftAssign
                | op @ ast::AssignOp::ZeroFillRShiftAssign
                | op @ ast::AssignOp::BitOrAssign
                | op @ ast::AssignOp::BitXorAssign
                | op @ ast::AssignOp::BitAndAssign
                | op @ ast::AssignOp::ExpAssign => {
                    let op = match op {
                        ast::AssignOp::AddAssign => ir::BinaryOp::Add,
                        ast::AssignOp::SubAssign => ir::BinaryOp::Sub,
                        ast::AssignOp::MulAssign => ir::BinaryOp::Mul,
                        ast::AssignOp::DivAssign => ir::BinaryOp::Div,
                        ast::AssignOp::ModAssign => ir::BinaryOp::Mod,
                        ast::AssignOp::LShiftAssign => ir::BinaryOp::ShiftLeft,
                        ast::AssignOp::RShiftAssign => ir::BinaryOp::ShiftRight,
                        ast::AssignOp::ZeroFillRShiftAssign => ir::BinaryOp::ShiftRightZero,
                        ast::AssignOp::BitOrAssign => ir::BinaryOp::BitOr,
                        ast::AssignOp::BitXorAssign => ir::BinaryOp::BitXor,
                        ast::AssignOp::BitAndAssign => ir::BinaryOp::BitAnd,
                        ast::AssignOp::ExpAssign => ir::BinaryOp::Exp,
                        _ => unreachable!(),
                    };
                    let left_ref = ir::Ref::new("_lhs");
                    stmts.push(ir::Stmt::Expr {
                        target: left_ref.clone(),
                        expr: read_expr,
                    });
                    let right_ref = ir::Ref::new("_rhs");
                    let (right_stmts, right_val) = convert_expression(*right, scope);
                    stmts.extend(right_stmts);
                    stmts.push(ir::Stmt::Expr {
                        target: right_ref.clone(),
                        expr: right_val,
                    });
                    stmts.push(ir::Stmt::Expr {
                        target: value_ref.clone(),
                        expr: ir::Expr::Binary {
                            op,
                            left: left_ref,
                            right: right_ref,
                        },
                    });
                    stmts.push(write_stmt);
                }
                ast::AssignOp::AndAssign
                | ast::AssignOp::OrAssign
                | ast::AssignOp::NullishAssign => {
                    unimplemented!("experimental assignment operators not supported")
                }
            }
            (stmts, ir::Expr::Read { source: value_ref })
        }
        ast::Expr::Member(ast::MemberExpr {
            obj,
            prop,
            computed,
            span: _,
        }) => {
            let obj_ref = ir::Ref::new("_obj");
            let (mut stmts, obj_value) = convert_expr_or_super(obj, scope);
            stmts.push(ir::Stmt::Expr {
                target: obj_ref.clone(),
                expr: obj_value,
            });
            let prop_ref = ir::Ref::new("_prp");
            let (prop_stmts, prop_value) = if computed {
                convert_expression(*prop, scope)
            } else {
                match *prop {
                    ast::Expr::Ident(ast::Ident {
                        sym,
                        span: _,
                        type_ann: _,
                        optional: _,
                    }) => (vec![], ir::Expr::String { value: sym }),
                    e => unreachable!("non-computed property is not an ident: {:?}", e),
                }
            };
            stmts.extend(prop_stmts);
            stmts.push(ir::Stmt::Expr {
                target: prop_ref.clone(),
                expr: prop_value,
            });
            (
                stmts,
                ir::Expr::ReadMember {
                    obj: obj_ref,
                    prop: prop_ref,
                },
            )
        }
        ast::Expr::Cond(ast::CondExpr {
            test,
            cons,
            alt,
            span: _,
        }) => {
            let test_ref = ir::Ref::new("_tst");
            let undef_ref = ir::Ref::new("_udf");
            let value_ref = ir::Ref::new("_val");
            let (mut stmts, test_value) = convert_expression(*test, scope);
            stmts.push(ir::Stmt::Expr {
                target: test_ref.clone(),
                expr: test_value,
            });
            stmts.push(ir::Stmt::Expr {
                target: undef_ref.clone(),
                expr: ir::Expr::Undefined,
            });
            stmts.push(ir::Stmt::DeclareMutable {
                target: value_ref.clone(),
                val: undef_ref,
            });
            stmts.push(ir::Stmt::IfElse {
                cond: test_ref,
                cons: {
                    let cons_ref = ir::Ref::new("_cns");
                    let (mut cons_stmts, cons_value) = convert_expression(*cons, scope);
                    cons_stmts.push(ir::Stmt::Expr {
                        target: cons_ref.clone(),
                        expr: cons_value,
                    });
                    cons_stmts.push(ir::Stmt::WriteMutable {
                        target: value_ref.clone(),
                        val: cons_ref,
                    });
                    ir::Block(cons_stmts)
                },
                alt: {
                    let alt_ref = ir::Ref::new("_alt");
                    let (mut alt_stmts, alt_value) = convert_expression(*alt, scope);
                    alt_stmts.push(ir::Stmt::Expr {
                        target: alt_ref.clone(),
                        expr: alt_value,
                    });
                    alt_stmts.push(ir::Stmt::WriteMutable {
                        target: value_ref.clone(),
                        val: alt_ref,
                    });
                    ir::Block(alt_stmts)
                },
            });
            (stmts, ir::Expr::ReadMutable { source: value_ref })
        }
        call_expr @ ast::Expr::Call(_) | call_expr @ ast::Expr::New(_) => {
            let (callee, arguments, call_kind) = match call_expr {
                ast::Expr::Call(ast::CallExpr {
                    callee,
                    args,
                    span: _,
                    type_args: _,
                }) => (callee, args, ir::CallKind::Call),
                ast::Expr::New(ast::NewExpr {
                    callee,
                    args,
                    span: _,
                    type_args: _,
                }) => (
                    ast::ExprOrSuper::Expr(callee),
                    args.unwrap_or_else(Vec::new),
                    ir::CallKind::New,
                ),
                _ => unreachable!(),
            };
            let is_direct_prop_call = match (&call_kind, &callee) {
                (ir::CallKind::Call, ast::ExprOrSuper::Expr(expr)) => match expr.as_ref() {
                    ast::Expr::Member(_) => true,
                    _ => false,
                },
                (ir::CallKind::Call, _) => false,
                (ir::CallKind::New, _) => false,
            };
            let (mut statements, callee_value) = convert_expr_or_super(callee, scope);
            let (base_ref, prop_ref) = match (is_direct_prop_call, callee_value) {
                (true, ir::Expr::ReadMember { obj, prop }) => (obj, Some(prop)),
                (true, _) => unreachable!("direct prop call receiver was not a read"),
                (false, callee_value) => {
                    let callee_ref = ir::Ref::new("_fun");
                    statements.push(ir::Stmt::Expr {
                        target: callee_ref.clone(),
                        expr: callee_value,
                    });
                    (callee_ref, None)
                }
            };
            let arguments = arguments
                .into_iter()
                .map(|ast::ExprOrSpread { spread, expr }| {
                    let kind = match spread {
                        Some(_) => ir::EleKind::Spread,
                        None => ir::EleKind::Single,
                    };
                    let arg_ref = ir::Ref::new("_arg");
                    let (stmts, arg_value) = convert_expression(*expr, scope);
                    statements.extend(stmts);
                    statements.push(ir::Stmt::Expr {
                        target: arg_ref.clone(),
                        expr: arg_value,
                    });
                    (kind, arg_ref)
                })
                .collect();
            (
                statements,
                ir::Expr::Call {
                    kind: call_kind,
                    base: base_ref,
                    prop: prop_ref,
                    args: arguments,
                },
            )
        }
        ast::Expr::Seq(ast::SeqExpr { exprs, span: _ }) => {
            let mut expressions: Vec<_> = exprs
                .into_iter()
                .map(|expr| convert_expression(*expr, scope))
                .collect();
            let last_expression = expressions
                .pop()
                .unwrap_or_else(|| unreachable!("empty SequenceExpression"));
            let mut statements = vec![];
            for (stmts, value) in expressions.into_iter() {
                statements.extend(stmts);
                statements.push(ir::Stmt::Expr {
                    target: ir::Ref::dead(),
                    expr: value,
                });
            }
            let (last_stmts, last_value) = last_expression;
            statements.extend(last_stmts);
            (statements, last_value)
        }
        ast::Expr::Paren(ast::ParenExpr { expr, span: _ }) => convert_expression(*expr, scope),
        ast::Expr::Tpl(_) | ast::Expr::TaggedTpl(_) => unimplemented!("templates not supported"),
        ast::Expr::Class(_) => unimplemented!("classes not supported"),
        ast::Expr::PrivateName(_) => unimplemented!("private members not supported"),
        ast::Expr::OptChain(_) => unimplemented!("optional chaining not supported"),
        ast::Expr::MetaProp(_) => unreachable!(),
        ast::Expr::JSXElement(_)
        | ast::Expr::JSXEmpty(_)
        | ast::Expr::JSXFragment(_)
        | ast::Expr::JSXMember(_)
        | ast::Expr::JSXNamespacedName(_) => unreachable!("jsx should not be parsed"),
        ast::Expr::TsTypeAssertion(_)
        | ast::Expr::TsConstAssertion(_)
        | ast::Expr::TsNonNull(_)
        | ast::Expr::TsTypeCast(_)
        | ast::Expr::TsAs(_) => unreachable!("ts should not be parsed"),
        ast::Expr::Invalid(_) => unreachable!(),
    }
}

fn convert_variable_declaration(var_decl: ast::VarDecl, scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    let ast::VarDecl {
        kind,
        decls,
        span: _,
        declare: _,
    } = var_decl;
    let mut stmts = vec![];
    for declarator in decls.into_iter() {
        let ast::VarDeclarator {
            name,
            init,
            span: _,
            definite: _,
        } = declarator;
        let init_ref = ir::Ref::new("_ini");
        match init {
            Some(init) => {
                let (init_stmts, init_value) = convert_expression(*init, scope);
                stmts.extend(init_stmts);
                stmts.push(ir::Stmt::Expr {
                    target: init_ref.clone(),
                    expr: init_value,
                });
            }
            None => {
                stmts.push(ir::Stmt::Expr {
                    target: init_ref.clone(),
                    expr: ir::Expr::Undefined,
                });
            }
        }
        let name = pat_to_ident(name);
        match kind {
            ast::VarDeclKind::Var => match scope.get_mutable(&name) {
                Some(var_ref) => stmts.push(ir::Stmt::WriteMutable {
                    target: var_ref.clone(),
                    val: init_ref,
                }),
                None => unreachable!("var not hoisted: {:?}", name),
            },
            ast::VarDeclKind::Let | ast::VarDeclKind::Const => match scope.get_mutable(&name) {
                Some(var_ref) => stmts.push(ir::Stmt::DeclareMutable {
                    target: var_ref.clone(),
                    val: init_ref,
                }),
                None => unreachable!("var not hoisted/predeclared: {:?}", name),
            },
        }
    }
    stmts
}

fn convert_expr_or_super(
    expr_or_super: ast::ExprOrSuper,
    scope: &scope::Ast,
) -> (Vec<ir::Stmt>, ir::Expr) {
    match expr_or_super {
        ast::ExprOrSuper::Expr(expr) => convert_expression(*expr, scope),
        ast::ExprOrSuper::Super(_) => unimplemented!("classes (and thus super) not supported"),
    }
}

fn pat_to_ident(pat: ast::Pat) -> JsWord {
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

fn propname_to_expr(propname: ast::PropName) -> ast::Expr {
    match propname {
        ast::PropName::Ident(ast::Ident {
            sym,
            span,
            type_ann: _,
            optional: _,
        }) => ast::Expr::Lit(ast::Lit::Str(ast::Str {
            span,
            value: sym,
            has_escape: false,
        })),
        ast::PropName::Str(s) => ast::Expr::Lit(ast::Lit::Str(s)),
        ast::PropName::Num(n) => ast::Expr::Lit(ast::Lit::Num(n)),
        ast::PropName::Computed(ast::ComputedPropName { expr, span: _ }) => *expr,
    }
}
