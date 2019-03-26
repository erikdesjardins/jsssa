use if_chain::if_chain;
use swc_atoms::JsWord;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;
use crate::swc_globals;
use crate::utils::P;

pub fn convert(_: &swc_globals::Initialized, ast: ast::Script) -> ir::Block {
    let ast::Script {
        shebang: _,
        body,
        span: _,
    } = ast;
    convert_block(body, &scope::Ast::default())
}

fn convert_block(body: Vec<ast::Stmt>, parent_scope: &scope::Ast) -> ir::Block {
    let mut scope = parent_scope.clone();

    let children = body
        .into_iter()
        .flat_map(|stmt| convert_statement(stmt, &mut scope))
        .collect();

    ir::Block { children }
}

fn convert_statement(stmt: ast::Stmt, scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    match stmt {
        ast::Stmt::Expr(expr) => {
            let (mut stmts, last_expr) = convert_expression(*expr, scope);
            stmts.push(ir::Stmt::Expr {
                target: ir::Ref::dead(),
                expr: last_expr,
            });
            stmts
        }
        ast::Stmt::Block(ast::BlockStmt { stmts, span: _ }) => vec![ir::Stmt::Block {
            body: P(convert_block(stmts, scope)),
        }],
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
        ast::Stmt::Labeled(_) => unimplemented!("labels not yet supported"),
        ast::Stmt::Break(ast::BreakStmt { label, span: _ }) => match label {
            Some(_) => unimplemented!("labels not yet supported"),
            None => vec![ir::Stmt::Break],
        },
        ast::Stmt::Continue(ast::ContinueStmt { label, span: _ }) => match label {
            Some(_) => unimplemented!("labels not yet supported"),
            None => vec![ir::Stmt::Continue],
        },
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
                cons: {
                    let children = convert_statement(*cons, &mut scope.clone());
                    P(ir::Block::with_children(children))
                },
                alt: match alt {
                    Some(alternate) => {
                        let children = convert_statement(*alternate, &mut scope.clone());
                        P(ir::Block::with_children(children))
                    }
                    None => P(ir::Block::empty()),
                },
            });
            stmts
        }
        ast::Stmt::Switch(_) => {
            // remember, switch cases aren't evaluated (!) until they're checked for equality
            unimplemented!("switch statements not yet supported")
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
                    P(convert_block(stmts, scope))
                },
                catch: match handler {
                    Some(ast::CatchClause {
                        param,
                        body,
                        span: _,
                    }) => {
                        let ast::BlockStmt { stmts, span: _ } = body;
                        let mut catch_scope = scope.clone();
                        let param = match param {
                            Some(param) => param,
                            None => unimplemented!("omitted catch binding not yet supported"),
                        };
                        let ref_ = convert_pattern(param, &mut catch_scope);
                        let mut block = convert_block(stmts, &catch_scope);
                        let desugar_param_ref = ir::Ref::new("_cat");
                        block.children.insert(
                            0,
                            ir::Stmt::Expr {
                                target: desugar_param_ref.clone(),
                                expr: ir::Expr::Argument { index: 0 },
                            },
                        );
                        block.children.insert(
                            1,
                            ir::Stmt::DeclareMutable {
                                target: ref_,
                                val: desugar_param_ref,
                            },
                        );
                        P(block)
                    }
                    None => P(ir::Block::empty()),
                },
                finally: match finalizer {
                    Some(ast::BlockStmt { stmts, span: _ }) => P(convert_block(stmts, scope)),
                    None => P(ir::Block::empty()),
                },
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
                cons: P(ir::Block::empty()),
                alt: P(ir::Block::with_children(vec![ir::Stmt::Break])),
            });
            let body_stmts = convert_statement(*body, &mut scope.clone());
            let stmts = if prefix {
                test_stmts.into_iter().chain(body_stmts)
            } else {
                body_stmts.into_iter().chain(test_stmts)
            }
            .collect();
            vec![ir::Stmt::Loop {
                body: P(ir::Block::with_children(stmts)),
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
                body: P(ir::Block::with_children({
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
                                cons: P(ir::Block::empty()),
                                alt: P(ir::Block::with_children(vec![ir::Stmt::Break])),
                            });
                            test_stmts
                        }
                        None => vec![],
                    };
                    stmts.extend(convert_statement(*body, &mut scope.clone()));
                    if let Some(update) = update {
                        let (update_stmts, update_value) = convert_expression(*update, scope);
                        stmts.extend(update_stmts);
                        stmts.push(ir::Stmt::Expr {
                            target: ir::Ref::dead(),
                            expr: update_value,
                        });
                    }
                    stmts
                })),
            });
            vec![ir::Stmt::Block {
                body: P(ir::Block::with_children(stmts)),
            }]
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
            let right_ref = ir::Ref::new("_rhs");
            let (mut stmts, right_value) = convert_expression(*right, scope);
            stmts.push(ir::Stmt::Expr {
                target: right_ref.clone(),
                expr: right_value,
            });
            let mut body_scope = scope.clone();
            let ele_binding = if_chain! {
                // todo we're definitely gonna need to use `kind`
                if let ast::VarDeclOrPat::VarDecl(ast::VarDecl { kind: _, decls, span: _, declare: _ }) = left;
                if decls.len() == 1;
                if let Some(ast::VarDeclarator { name, init: None, span: _, definite: _ }) = decls.into_iter().next();
                then {
                    convert_pattern(name, &mut body_scope)
                } else {
                    unimplemented!("for in/of statements with complex initializers not supported");
                }
            };
            let mut body_stmts = convert_statement(*body, &mut body_scope);
            let desugar_param_ref = ir::Ref::new("_for");
            body_stmts.insert(
                0,
                ir::Stmt::Expr {
                    target: desugar_param_ref.clone(),
                    expr: ir::Expr::Argument { index: 0 },
                },
            );
            body_stmts.insert(
                1,
                ir::Stmt::DeclareMutable {
                    target: ele_binding,
                    val: desugar_param_ref,
                },
            );
            stmts.push(ir::Stmt::ForEach {
                kind,
                init: right_ref,
                body: P(ir::Block::with_children(body_stmts)),
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
                let mut fn_scope = scope.clone();
                let param_refs = params
                    .into_iter()
                    .map(|param| convert_pattern(param, &mut fn_scope))
                    .collect::<Vec<_>>();
                let recursive_ref = ir::Ref::new(sym.clone());
                fn_scope.declare_mutable(sym.clone(), recursive_ref.clone());
                let body = match body {
                    Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                    None => unreachable!("bodyless function type declaration"),
                };
                let ir::Block { children } = convert_block(body, &fn_scope);
                let desugar_ref = ir::Ref::new(recursive_ref.name_hint());
                let cur_fn = vec![
                    ir::Stmt::Expr {
                        target: desugar_ref.clone(),
                        expr: ir::Expr::CurrentFunction,
                    },
                    ir::Stmt::DeclareMutable {
                        target: recursive_ref,
                        val: desugar_ref,
                    },
                ];
                let params = param_refs.into_iter().enumerate().flat_map(|(i, arg)| {
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
                });
                let block = ir::Block::with_children(
                    cur_fn.into_iter().chain(params).chain(children).collect(),
                );
                let fn_ref = ir::Ref::new("_fun");
                let fn_binding = ir::Ref::new(sym.clone());
                scope.declare_mutable(sym, fn_binding.clone());
                vec![
                    ir::Stmt::Expr {
                        target: fn_ref.clone(),
                        expr: ir::Expr::Function {
                            kind: ir::FnKind::Func {
                                is_async,
                                is_generator,
                            },
                            body: P(block),
                        },
                    },
                    ir::Stmt::DeclareMutable {
                        target: fn_binding,
                        val: fn_ref,
                    },
                ]
            }
            ast::Decl::Var(var_decl) => convert_variable_declaration(var_decl, scope),
            ast::Decl::Class(_) => unimplemented!("classes not yet supported"),
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
                exp:
                    ast::Str {
                        value,
                        has_escape,
                        span: _,
                    },
                flags,
                span: _,
            }) => (
                vec![],
                ir::Expr::RegExp {
                    regex: value,
                    has_escape,
                    flags: match flags {
                        Some(ast::Str {
                            value,
                            has_escape,
                            span: _,
                        }) => {
                            assert!(!has_escape);
                            value
                        }
                        None => JsWord::from(""),
                    },
                },
            ),
            ast::Lit::Null(ast::Null { span: _ }) => (vec![], ir::Expr::Null),
            ast::Lit::Str(ast::Str {
                value,
                has_escape,
                span: _,
            }) => (vec![], ir::Expr::String { value, has_escape }),
            ast::Lit::Bool(ast::Bool { value, span: _ }) => (vec![], ir::Expr::Bool { value }),
            ast::Lit::Num(ast::Number { value, span: _ }) => (vec![], ir::Expr::Number { value }),
            ast::Lit::JSXText(_) => unreachable!(),
        },
        ast::Expr::This(ast::ThisExpr { span: _ }) => (vec![], ir::Expr::This),
        ast::Expr::Arrow(ast::ArrowExpr {
            params,
            body,
            is_async,
            is_generator,
            span: _,
            type_params: _,
            return_type: _,
        }) => {
            let mut fn_scope = scope.clone();
            let param_refs = params
                .into_iter()
                .map(|param| convert_pattern(param, &mut fn_scope))
                .collect::<Vec<_>>();
            let ir::Block { children } = match body {
                ast::BlockStmtOrExpr::BlockStmt(block) => {
                    let ast::BlockStmt { stmts, span: _ } = block;
                    convert_block(stmts, &fn_scope)
                }
                ast::BlockStmtOrExpr::Expr(expr) => {
                    let ref_ = ir::Ref::new("_arr");
                    let (mut stmts, return_value) = convert_expression(*expr, &fn_scope);
                    stmts.push(ir::Stmt::Expr {
                        target: ref_.clone(),
                        expr: return_value,
                    });
                    stmts.push(ir::Stmt::Return { val: ref_ });
                    ir::Block::with_children(stmts)
                }
            };
            let params = param_refs.into_iter().enumerate().flat_map(|(i, arg)| {
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
            });
            let body = ir::Block::with_children(params.chain(children).collect());
            assert!(!is_generator, "generator arrow function");
            let func = ir::Expr::Function {
                kind: ir::FnKind::Arrow { is_async },
                body: P(body),
            };
            (vec![], func)
        }
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
                                        value: sym.clone(),
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
                                ast::Prop::Getter(ast::GetterProp { key, body, span }) => (
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
                                        params: vec![param],
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

                            let mut fn_scope = scope.clone();
                            let param_refs = params
                                .into_iter()
                                .map(|param| convert_pattern(param, &mut fn_scope))
                                .collect::<Vec<_>>();
                            let body = match block {
                                Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                                None => unreachable!("object method/accessor without body"),
                            };
                            let ir::Block { children } = convert_block(body, &fn_scope);
                            let params = param_refs.into_iter().enumerate().flat_map(|(i, arg)| {
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
                            });
                            let body = ir::Block::with_children(params.chain(children).collect());
                            let fn_value = ir::Expr::Function {
                                kind: ir::FnKind::Func {
                                    is_async,
                                    is_generator,
                                },
                                body: P(body),
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
            let mut fn_scope = scope.clone();
            let param_refs = params
                .into_iter()
                .map(|param| convert_pattern(param, &mut fn_scope))
                .collect::<Vec<_>>();
            let recursive_ref = match ident {
                Some(ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                }) => {
                    let recursive_ref = ir::Ref::new(sym.clone());
                    fn_scope.declare_mutable(sym, recursive_ref.clone());
                    Some(recursive_ref)
                }
                None => None,
            };
            let body = match body {
                Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                None => unreachable!("bodyless function type declaration"),
            };
            let ir::Block { children } = convert_block(body, &fn_scope);
            let params = param_refs.into_iter().enumerate().flat_map(|(i, arg)| {
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
            });
            let mut block = ir::Block::with_children(params.chain(children).collect());
            if let Some(recursive_ref) = recursive_ref {
                let desugar_ref = ir::Ref::new(recursive_ref.name_hint());
                block.children.insert(
                    0,
                    ir::Stmt::Expr {
                        target: desugar_ref.clone(),
                        expr: ir::Expr::CurrentFunction,
                    },
                );
                block.children.insert(
                    1,
                    ir::Stmt::DeclareMutable {
                        target: recursive_ref,
                        val: desugar_ref,
                    },
                );
            }

            let func = ir::Expr::Function {
                kind: ir::FnKind::Func {
                    is_async,
                    is_generator,
                },
                body: P(block),
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
                                }) => (
                                    vec![],
                                    ir::Expr::String {
                                        value: sym,
                                        has_escape: false,
                                    },
                                ),
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
                    expr: ir::Expr::Number { value: 1.0 },
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
                        let full = ir::Block::with_children(right_stmts);
                        let empty = ir::Block::empty();
                        match op {
                            ast::BinaryOp::LogicalOr => (empty, full),
                            ast::BinaryOp::LogicalAnd => (full, empty),
                            _ => unreachable!(),
                        }
                    };
                    stmts.push(ir::Stmt::IfElse {
                        cond: left_ref,
                        cons: P(consequent),
                        alt: P(alternate),
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
            let value_ref = ir::Ref::new("_val");
            let (mut stmts, read_expr, write_stmt) = match left {
                ast::PatOrExpr::Pat(pat) => match *pat {
                    ast::Pat::Ident(ast::Ident {
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
                    ast::Pat::Expr(expr) => match *expr {
                        ast::Expr::Member(ast::MemberExpr {
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
                                    }) => (
                                        vec![],
                                        ir::Expr::String {
                                            value: sym,
                                            has_escape: false,
                                        },
                                    ),
                                    e => unreachable!(
                                        "non-computed property is not an ident: {:?}",
                                        e
                                    ),
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
                        _ => {
                            unimplemented!("assigning to non member-expression expression pattern")
                        }
                    },
                    pat => {
                        unimplemented!("assigning to complex patterns not yet supported: {:?}", pat)
                    }
                },
                ast::PatOrExpr::Expr(_) => {
                    unreachable!("assigning to expression");
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
                        ast::AssignOp::Assign => unreachable!(),
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
                    }) => (
                        vec![],
                        ir::Expr::String {
                            value: sym,
                            has_escape: false,
                        },
                    ),
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
                    let alt_ref = ir::Ref::new("_cns");
                    let (mut alt_stmts, alt_value) = convert_expression(*alt, scope);
                    alt_stmts.push(ir::Stmt::Expr {
                        target: alt_ref.clone(),
                        expr: alt_value,
                    });
                    alt_stmts.push(ir::Stmt::WriteMutable {
                        target: value_ref.clone(),
                        val: alt_ref,
                    });
                    P(ir::Block::with_children(alt_stmts))
                },
                alt: {
                    let cons_ref = ir::Ref::new("_alt");
                    let (mut cons_stmts, cons_value) = convert_expression(*cons, scope);
                    cons_stmts.push(ir::Stmt::Expr {
                        target: cons_ref.clone(),
                        expr: cons_value,
                    });
                    cons_stmts.push(ir::Stmt::WriteMutable {
                        target: value_ref.clone(),
                        val: cons_ref,
                    });
                    P(ir::Block::with_children(cons_stmts))
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
            let callee_ref = ir::Ref::new("_fun");
            let (mut statements, callee_value) = convert_expr_or_super(callee, scope);
            statements.push(ir::Stmt::Expr {
                target: callee_ref.clone(),
                expr: callee_value,
            });
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
                    func: callee_ref,
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
        ast::Expr::Tpl(_) | ast::Expr::TaggedTpl(_) => {
            unimplemented!("templates not yet supported")
        }
        ast::Expr::Class(_) => unimplemented!("classes not yet supported"),
        ast::Expr::PrivateName(_) => unimplemented!("private members not yet supported"),
        ast::Expr::MetaProp(_) => unreachable!(),
        ast::Expr::JSXElement(_)
        | ast::Expr::JSXEmpty(_)
        | ast::Expr::JSXFragment(_)
        | ast::Expr::JSXMebmer(_)
        | ast::Expr::JSXNamespacedName(_) => unreachable!(),
        ast::Expr::TsTypeAssertion(_)
        | ast::Expr::TsNonNull(_)
        | ast::Expr::TsTypeCast(_)
        | ast::Expr::TsAs(_) => unreachable!(),
    }
}

fn convert_variable_declaration(var_decl: ast::VarDecl, scope: &mut scope::Ast) -> Vec<ir::Stmt> {
    // todo we're definitely gonna need to handle `kind`
    let ast::VarDecl {
        kind: _,
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
        let var_ref = convert_pattern(name, scope);
        stmts.push(ir::Stmt::DeclareMutable {
            target: var_ref,
            val: init_ref,
        });
    }
    stmts
}

fn convert_pattern(pat: ast::Pat, scope: &mut scope::Ast) -> ir::Ref<ir::Mutable> {
    match pat {
        ast::Pat::Ident(ast::Ident {
            sym,
            span: _,
            type_ann: _,
            optional: _,
        }) => {
            let ref_ = ir::Ref::new(sym.clone());
            scope.declare_mutable(sym, ref_.clone());
            ref_
        }
        ast::Pat::Array(_)
        | ast::Pat::Object(_)
        | ast::Pat::Rest(_)
        | ast::Pat::Assign(_)
        | ast::Pat::Expr(_) => unimplemented!("complex patterns not yet supported"),
    }
}

fn convert_expr_or_super(
    expr_or_super: ast::ExprOrSuper,
    scope: &scope::Ast,
) -> (Vec<ir::Stmt>, ir::Expr) {
    match expr_or_super {
        ast::ExprOrSuper::Expr(expr) => convert_expression(*expr, scope),
        ast::ExprOrSuper::Super(_) => unimplemented!("classes (and thus super) not yet supported"),
    }
}

fn propname_to_expr(propname: ast::PropName) -> ast::Expr {
    match propname {
        ast::PropName::Ident(i) => ast::Expr::Ident(i),
        ast::PropName::Str(s) => ast::Expr::Lit(ast::Lit::Str(s)),
        ast::PropName::Num(n) => ast::Expr::Lit(ast::Lit::Num(n)),
        ast::PropName::Computed(e) => *e,
    }
}
