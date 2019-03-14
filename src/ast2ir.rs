use std::collections::BTreeMap;

use if_chain::if_chain;
use swc_atoms::JsWord;
use swc_ecma_ast as ast;

use crate::ir;
use crate::utils::P;

type ScopeMap = BTreeMap<JsWord, ir::Ref<ir::Mutable>>;

pub fn convert(ast: Vec<ast::Stmt>) -> ir::Block {
    convert_block(ast, &BTreeMap::default())
}

fn convert_block(body: Vec<ast::Stmt>, parent_scopes: &ScopeMap) -> ir::Block {
    let mut scope = parent_scopes.clone();

    let children = body
        .into_iter()
        .flat_map(|stmt| convert_statement(stmt, &mut scope))
        .collect();

    ir::Block { children }
}

fn convert_statement(stmt: ast::Stmt, scope: &mut ScopeMap) -> Vec<ir::Stmt> {
    match stmt {
        ast::Stmt::Expr(expr) => {
            let (mut stmts, last_expr) = convert_expression(*expr, scope);
            stmts.push(ir::Stmt::Expr(ir::Ref::Dead, last_expr));
            stmts
        }
        ast::Stmt::Block(ast::BlockStmt { stmts, span: _ }) => {
            vec![ir::Stmt::Block(P(convert_block(stmts, scope)))]
        }
        ast::Stmt::Empty(ast::EmptyStmt { span: _ }) => vec![],
        ast::Stmt::Debugger(ast::DebuggerStmt { span: _ }) => vec![ir::Stmt::Debugger],
        ast::Stmt::With(_) => unimplemented!("with() statement not supported"),
        ast::Stmt::Return(ast::ReturnStmt { arg, span: _ }) => {
            let ref_ = ir::Ref::new("return_");
            match arg {
                Some(arg) => {
                    let (mut stmts, return_value) = convert_expression(*arg, scope);
                    stmts.push(ir::Stmt::Expr(ref_.clone(), return_value));
                    stmts.push(ir::Stmt::Return(ref_));
                    stmts
                }
                None => vec![
                    ir::Stmt::Expr(ref_.clone(), ir::Expr::Undefined),
                    ir::Stmt::Return(ref_),
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
            let ref_ = ir::Ref::new("if_");
            let (mut stmts, test_value) = convert_expression(*test, scope);
            stmts.push(ir::Stmt::Expr(ref_.clone(), test_value));
            stmts.push(ir::Stmt::IfElse(
                ref_,
                {
                    let children = convert_statement(*cons, &mut scope.clone());
                    P(ir::Block::with_children(children))
                },
                match alt {
                    Some(alternate) => {
                        let children = convert_statement(*alternate, &mut scope.clone());
                        P(ir::Block::with_children(children))
                    }
                    None => P(ir::Block::empty()),
                },
            ));
            stmts
        }
        ast::Stmt::Switch(_) =>
        // remember, switch cases aren't evaluated (!) until they're checked for equality
        {
            unimplemented!("switch statements not yet supported")
        }
        ast::Stmt::Throw(ast::ThrowStmt { arg, span: _ }) => {
            let ref_ = ir::Ref::new("throw_");
            let (mut stmts, throw_value) = convert_expression(*arg, scope);
            stmts.push(ir::Stmt::Expr(ref_.clone(), throw_value));
            stmts.push(ir::Stmt::Throw(ref_));
            stmts
        }
        ast::Stmt::Try(ast::TryStmt {
            block,
            handler,
            finalizer,
            span: _,
        }) => {
            let try_ = ir::Stmt::Try(
                {
                    let ast::BlockStmt { stmts, span: _ } = block;
                    P(convert_block(stmts, scope))
                },
                handler.map(
                    |ast::CatchClause {
                         param,
                         body,
                         span: _,
                     }| {
                        let ast::BlockStmt { stmts, span: _ } = body;
                        let mut catch_scope = scope.clone();
                        let param = match param {
                            Some(param) => param,
                            None => unimplemented!("omitted catch binding not yet supported"),
                        };
                        let ref_ = convert_pattern(param, &mut catch_scope);
                        (ref_, P(convert_block(stmts, &catch_scope)))
                    },
                ),
                finalizer.map(|finalizer| {
                    let ast::BlockStmt { stmts, span: _ } = finalizer;
                    P(convert_block(stmts, scope))
                }),
            );
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
            let cond_ref = ir::Ref::new("while_");
            let (mut test_stmts, test_value) = convert_expression(*test, scope);
            test_stmts.push(ir::Stmt::Expr(cond_ref.clone(), test_value));
            test_stmts.push(ir::Stmt::IfElse(
                cond_ref,
                P(ir::Block::empty()),
                P(ir::Block::with_children(vec![ir::Stmt::Break])),
            ));
            let body_stmts = convert_statement(*body, &mut scope.clone());
            let stmts = if prefix {
                test_stmts.into_iter().chain(body_stmts)
            } else {
                body_stmts.into_iter().chain(test_stmts)
            }
            .collect();
            vec![ir::Stmt::Loop(P(ir::Block::with_children(stmts)))]
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
                    init_stmts.push(ir::Stmt::Expr(ir::Ref::Dead, init_value));
                    init_stmts
                }
                Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
                    convert_variable_declaration(var_decl, scope)
                }
                None => vec![],
            };
            stmts.push(ir::Stmt::Loop(P(ir::Block::with_children({
                let mut stmts = match test {
                    Some(test) => {
                        let cond_ref = ir::Ref::new("for_");
                        let (mut test_stmts, test_value) = convert_expression(*test, scope);
                        test_stmts.push(ir::Stmt::Expr(cond_ref.clone(), test_value));
                        test_stmts.push(ir::Stmt::IfElse(
                            cond_ref,
                            P(ir::Block::empty()),
                            P(ir::Block::with_children(vec![ir::Stmt::Break])),
                        ));
                        test_stmts
                    }
                    None => vec![],
                };
                stmts.extend(convert_statement(*body, &mut scope.clone()));
                if let Some(update) = update {
                    let (update_stmts, update_value) = convert_expression(*update, scope);
                    stmts.extend(update_stmts);
                    stmts.push(ir::Stmt::Expr(ir::Ref::Dead, update_value));
                }
                stmts
            }))));
            vec![ir::Stmt::Block(P(ir::Block::with_children(stmts)))]
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
            let right_ref = ir::Ref::new("right_");
            let (mut stmts, right_value) = convert_expression(*right, scope);
            stmts.push(ir::Stmt::Expr(right_ref.clone(), right_value));
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
            let body_stmts = convert_statement(*body, &mut body_scope);
            stmts.push(ir::Stmt::For(
                kind,
                ele_binding,
                right_ref,
                P(ir::Block::with_children(body_stmts)),
            ));
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
                let refs = params
                    .into_iter()
                    .map(|param| convert_pattern(param, &mut fn_scope))
                    .collect();
                let recursive_ref = ir::Ref::new(sym.clone());
                fn_scope.insert(sym.clone(), recursive_ref.clone());
                let body = match body {
                    Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                    None => unreachable!("bodyless function type declaration"),
                };
                let mut block = convert_block(body, &fn_scope);
                let desugar_ref = ir::Ref::new("curfn_");
                // todo this double writing (and below for FnExpr) may be unnecessary with improved binding support,
                // since it can be brought into scope
                block.children.insert(
                    0,
                    ir::Stmt::Expr(desugar_ref.clone(), ir::Expr::CurrentFunction),
                );
                block
                    .children
                    .insert(1, ir::Stmt::WriteBinding(recursive_ref, desugar_ref));
                let fn_ref = ir::Ref::new("fn_");
                let fn_binding = ir::Ref::new(sym.clone());
                vec![
                    ir::Stmt::Expr(
                        fn_ref.clone(),
                        ir::Expr::Function(
                            ir::FnKind::Func {
                                is_async,
                                is_generator,
                            },
                            Some(sym),
                            refs,
                            P(block),
                        ),
                    ),
                    ir::Stmt::WriteBinding(fn_binding, fn_ref),
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

fn convert_expression(expr: ast::Expr, scope: &ScopeMap) -> (Vec<ir::Stmt>, ir::Expr) {
    match expr {
        ast::Expr::Ident(ast::Ident {
            sym,
            span: _,
            type_ann: _,
            optional: _,
        }) => {
            let expr = match scope.get(&sym) {
                Some(ref_) => ir::Expr::ReadBinding(ref_.clone()),
                None => ir::Expr::ReadGlobal(sym),
            };
            (vec![], expr)
        }
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Regex(ast::Regex {
                exp,
                flags,
                span: _,
            }) => (vec![], ir::Expr::RegExp(exp.value, flags.map(|s| s.value))),
            ast::Lit::Null(ast::Null { span: _ }) => (vec![], ir::Expr::Null),
            ast::Lit::Str(ast::Str {
                value,
                span: _,
                has_escape: _,
            }) => (vec![], ir::Expr::String(value)),
            ast::Lit::Bool(ast::Bool { value, span: _ }) => (vec![], ir::Expr::Bool(value)),
            ast::Lit::Num(ast::Number { value, span: _ }) => (vec![], ir::Expr::Number(value)),
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
            let refs = params
                .into_iter()
                .map(|param| convert_pattern(param, &mut fn_scope))
                .collect();
            let body = match body {
                ast::BlockStmtOrExpr::BlockStmt(block) => {
                    let ast::BlockStmt { stmts, span: _ } = block;
                    convert_block(stmts, &fn_scope)
                }
                ast::BlockStmtOrExpr::Expr(expr) => {
                    let ref_ = ir::Ref::new("arrow_");
                    let (mut stmts, return_value) = convert_expression(*expr, &fn_scope);
                    stmts.push(ir::Stmt::Expr(ref_.clone(), return_value));
                    stmts.push(ir::Stmt::Return(ref_));
                    ir::Block::with_children(stmts)
                }
            };
            assert!(!is_generator, "generator arrow function");
            let func = ir::Expr::Function(ir::FnKind::Arrow { is_async }, None, refs, P(body));
            (vec![], func)
        }
        ast::Expr::Yield(ast::YieldExpr {
            arg,
            delegate,
            span: _,
        }) => {
            let ref_ = ir::Ref::new("yield_");
            let (mut stmts, yield_value) = match arg {
                Some(argument) => convert_expression(*argument, scope),
                None => (vec![], ir::Expr::Undefined),
            };
            stmts.push(ir::Stmt::Expr(ref_.clone(), yield_value));
            let kind = if delegate {
                ir::YieldKind::Delegate
            } else {
                ir::YieldKind::Single
            };
            (stmts, ir::Expr::Yield(kind, ref_))
        }
        ast::Expr::Await(ast::AwaitExpr { arg, span: _ }) => {
            let ref_ = ir::Ref::new("await_");
            let (mut stmts, await_value) = convert_expression(*arg, scope);
            stmts.push(ir::Stmt::Expr(ref_.clone(), await_value));
            (stmts, ir::Expr::Await(ref_))
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
                        let ref_ = ir::Ref::new("ele_");
                        let (stmts, ele_value) = convert_expression(*expr, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Expr(ref_.clone(), ele_value));
                        (kind, ref_)
                    })
                })
                .collect();
            (statements, ir::Expr::Array(elements))
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
                                        span: span.clone(),
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
                            let ref_key = ir::Ref::new("key_");
                            let (stmts, key_value) = convert_expression(key, scope);
                            statements.extend(stmts);
                            statements.push(ir::Stmt::Expr(ref_key.clone(), key_value));
                            let ref_value = ir::Ref::new("value_");
                            let (stmts, value_value) = convert_expression(value, scope);
                            statements.extend(stmts);
                            statements.push(ir::Stmt::Expr(ref_value.clone(), value_value));
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

                            let ref_key = ir::Ref::new("key_");
                            let (stmts, key_value) = convert_expression(key, scope);
                            statements.extend(stmts);
                            statements.push(ir::Stmt::Expr(ref_key.clone(), key_value));

                            let mut fn_scope = scope.clone();
                            let param_refs = params
                                .into_iter()
                                .map(|param| convert_pattern(param, &mut fn_scope))
                                .collect();
                            let body = match block {
                                Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                                None => unreachable!("object method/accessor without body"),
                            };
                            let body = convert_block(body, &fn_scope);
                            let fn_value = ir::Expr::Function(
                                ir::FnKind::Func {
                                    is_async,
                                    is_generator,
                                },
                                None,
                                param_refs,
                                P(body),
                            );
                            let ref_value = ir::Ref::new("value_");
                            statements.push(ir::Stmt::Expr(ref_value.clone(), fn_value));

                            (kind, ref_key, ref_value)
                        }
                        ast::Prop::Assign(_) => unreachable!("assignment prop in object literal"),
                    },
                    ast::PropOrSpread::Spread(_) => unimplemented!("object spread not implemented"),
                })
                .collect();
            (statements, ir::Expr::Object(properties))
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
            let sym = ident.map(
                |ast::Ident {
                     sym,
                     span: _,
                     type_ann: _,
                     optional: _,
                 }| sym,
            );
            let mut fn_scope = scope.clone();
            let refs = params
                .into_iter()
                .map(|param| convert_pattern(param, &mut fn_scope))
                .collect();
            let recursive_ref = match &sym {
                Some(sym) => {
                    let recursive_ref = ir::Ref::new(sym.clone());
                    fn_scope.insert(sym.clone(), recursive_ref.clone());
                    Some(recursive_ref)
                }
                None => None,
            };
            let body = match body {
                Some(ast::BlockStmt { stmts, span: _ }) => stmts,
                None => unreachable!("bodyless function type declaration"),
            };
            let mut block = convert_block(body, &fn_scope);
            if let Some(recursive_ref) = recursive_ref {
                let desugar_ref = ir::Ref::new("curfn_");
                block.children.insert(
                    0,
                    ir::Stmt::Expr(desugar_ref.clone(), ir::Expr::CurrentFunction),
                );
                block
                    .children
                    .insert(1, ir::Stmt::WriteBinding(recursive_ref, desugar_ref));
            }

            let func = ir::Expr::Function(
                ir::FnKind::Func {
                    is_async,
                    is_generator,
                },
                sym,
                refs,
                P(block),
            );
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
                            // todo we need to use this to avoid `foo.prop` becoming `foo[prop]`
                            computed: _,
                            span: _,
                        } = expr;
                        let obj_ref = ir::Ref::new("obj_");
                        let (mut stmts, obj_value) = convert_expr_or_super(obj, scope);
                        stmts.push(ir::Stmt::Expr(obj_ref.clone(), obj_value));
                        let prop_ref = ir::Ref::new("prop_");
                        let (prop_stmts, prop_value) = convert_expression(*prop, scope);
                        stmts.extend(prop_stmts);
                        stmts.push(ir::Stmt::Expr(prop_ref.clone(), prop_value));
                        return (stmts, ir::Expr::Delete(obj_ref, prop_ref));
                    }
                    _ => unimplemented!("deletion of non-MemberExpression not supported"),
                },
            };
            let ref_ = ir::Ref::new("unary_");
            let (mut stmts, expr_value) = convert_expression(*arg, scope);
            stmts.push(ir::Stmt::Expr(ref_.clone(), expr_value));
            (stmts, ir::Expr::Unary(op, ref_))
        }
        ast::Expr::Update(ast::UpdateExpr {
            op,
            arg,
            prefix,
            span: _,
        }) => {
            let one_ref = ir::Ref::new("one_");
            let read_ref = ir::Ref::new("read_");
            let write_ref = ir::Ref::new("write_");
            let (read, write) = match *arg {
                ast::Expr::Ident(ast::Ident {
                    sym,
                    span: _,
                    type_ann: _,
                    optional: _,
                }) => match scope.get(&sym) {
                    Some(ref_) => (
                        ir::Expr::ReadBinding(ref_.clone()),
                        ir::Stmt::WriteBinding(ref_.clone(), write_ref.clone()),
                    ),
                    None => (
                        ir::Expr::ReadGlobal(sym.clone()),
                        ir::Stmt::WriteGlobal(sym, write_ref.clone()),
                    ),
                },
                arg => unimplemented!("unexpected UpdateExpression argument: {:?}", arg),
            };
            let op = match op {
                ast::UpdateOp::PlusPlus => ir::BinaryOp::Add,
                ast::UpdateOp::MinusMinus => ir::BinaryOp::Sub,
            };
            let stmts = vec![
                ir::Stmt::Expr(read_ref.clone(), read),
                ir::Stmt::Expr(one_ref.clone(), ir::Expr::Number(1.0)),
                ir::Stmt::Expr(
                    write_ref.clone(),
                    ir::Expr::Binary(op, read_ref.clone(), one_ref),
                ),
                write,
            ];
            let value = if prefix { write_ref } else { read_ref };
            (stmts, ir::Expr::Read(value))
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
                    let left_ref = ir::Ref::new("pred_");
                    let value_ref = ir::Ref::new("logi_");
                    let (mut stmts, left_value) = convert_expression(*left, scope);
                    stmts.push(ir::Stmt::Expr(left_ref.clone(), left_value));
                    stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), left_ref.clone()));
                    let (consequent, alternate) = {
                        let right_ref = ir::Ref::new("cons_");
                        let (mut right_stmts, right_value) = convert_expression(*right, scope);
                        right_stmts.push(ir::Stmt::Expr(right_ref.clone(), right_value));
                        right_stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), right_ref));
                        let full = ir::Block::with_children(right_stmts);
                        let empty = ir::Block::empty();
                        match op {
                            ast::BinaryOp::LogicalOr => (empty, full),
                            ast::BinaryOp::LogicalAnd => (full, empty),
                            _ => unreachable!(),
                        }
                    };
                    stmts.push(ir::Stmt::IfElse(left_ref, P(consequent), P(alternate)));
                    (stmts, ir::Expr::ReadBinding(value_ref))
                }
                op => {
                    let op = match op {
                        ast::BinaryOp::EqEq => ir::BinaryOp::EqEq,
                        ast::BinaryOp::NotEq => ir::BinaryOp::NotEq,
                        ast::BinaryOp::EqEqEq => ir::BinaryOp::StrictEq,
                        ast::BinaryOp::NotEqEq => ir::BinaryOp::NotStrictEq,
                        ast::BinaryOp::Lt => ir::BinaryOp::Lt,
                        ast::BinaryOp::LtEq => ir::BinaryOp::Lte,
                        ast::BinaryOp::Gt => ir::BinaryOp::Gt,
                        ast::BinaryOp::GtEq => ir::BinaryOp::Gte,
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
                    let left_ref = ir::Ref::new("left_");
                    let (mut stmts, left_value) = convert_expression(*left, scope);
                    stmts.push(ir::Stmt::Expr(left_ref.clone(), left_value));
                    let right_ref = ir::Ref::new("right_");
                    let (right_stmts, right_value) = convert_expression(*right, scope);
                    stmts.extend(right_stmts);
                    stmts.push(ir::Stmt::Expr(right_ref.clone(), right_value));
                    (stmts, ir::Expr::Binary(op, left_ref, right_ref))
                }
            }
        }
        ast::Expr::Assign(ast::AssignExpr {
            op,
            left,
            right,
            span: _,
        }) => {
            let value_ref = ir::Ref::new("val_");
            let (mut stmts, read_expr, write_stmt) = match left {
                ast::PatOrExpr::Pat(pat) => match *pat {
                    ast::Pat::Ident(ast::Ident {
                        sym,
                        span: _,
                        type_ann: _,
                        optional: _,
                    }) => match scope.get(&sym) {
                        Some(binding) => (
                            vec![],
                            ir::Expr::ReadBinding(binding.clone()),
                            ir::Stmt::WriteBinding(binding.clone(), value_ref.clone()),
                        ),
                        None => (
                            vec![],
                            ir::Expr::ReadGlobal(sym.clone()),
                            ir::Stmt::WriteGlobal(sym.clone(), value_ref.clone()),
                        ),
                    },
                    ast::Pat::Expr(expr) => match *expr {
                        ast::Expr::Member(ast::MemberExpr {
                            obj,
                            prop,
                            // todo we need to use this to avoid `foo.prop` becoming `foo[prop]`
                            computed: _,
                            span: _,
                        }) => {
                            let obj_ref = ir::Ref::new("obj_");
                            let prop_ref = ir::Ref::new("prop_");
                            let (mut stmts, obj_value) = convert_expr_or_super(obj, scope);
                            stmts.push(ir::Stmt::Expr(obj_ref.clone(), obj_value));
                            let (prop_stmts, prop_value) = convert_expression(*prop, scope);
                            stmts.extend(prop_stmts);
                            stmts.push(ir::Stmt::Expr(prop_ref.clone(), prop_value));
                            (
                                stmts,
                                ir::Expr::ReadMember(obj_ref.clone(), prop_ref.clone()),
                                ir::Stmt::WriteMember(obj_ref, prop_ref, value_ref.clone()),
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
                    stmts.push(ir::Stmt::Expr(value_ref.clone(), right_val));
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
                    let left_ref = ir::Ref::new("left_");
                    stmts.push(ir::Stmt::Expr(left_ref.clone(), read_expr));
                    let right_ref = ir::Ref::new("right_");
                    let (right_stmts, right_val) = convert_expression(*right, scope);
                    stmts.extend(right_stmts);
                    stmts.push(ir::Stmt::Expr(right_ref.clone(), right_val));
                    stmts.push(ir::Stmt::Expr(
                        value_ref.clone(),
                        ir::Expr::Binary(op, left_ref, right_ref),
                    ));
                    stmts.push(write_stmt);
                }
            }
            (stmts, ir::Expr::Read(value_ref))
        }
        ast::Expr::Member(ast::MemberExpr {
            obj,
            prop,
            // todo we need to use this to avoid `foo.prop` becoming `foo[prop]`
            computed: _,
            span: _,
        }) => {
            let obj_ref = ir::Ref::new("obj_");
            let (mut stmts, obj_value) = convert_expr_or_super(obj, scope);
            stmts.push(ir::Stmt::Expr(obj_ref.clone(), obj_value));
            let prop_ref = ir::Ref::new("prop_");
            let (prop_stmts, prop_value) = convert_expression(*prop, scope);
            stmts.extend(prop_stmts);
            stmts.push(ir::Stmt::Expr(prop_ref.clone(), prop_value));
            (stmts, ir::Expr::ReadMember(obj_ref, prop_ref))
        }
        ast::Expr::Cond(ast::CondExpr {
            test,
            cons,
            alt,
            span: _,
        }) => {
            let test_ref = ir::Ref::new("test_");
            let undef_ref = ir::Ref::new("undef_");
            let value_ref = ir::Ref::new("value_");
            let (mut stmts, test_value) = convert_expression(*test, scope);
            stmts.push(ir::Stmt::Expr(test_ref.clone(), test_value));
            stmts.push(ir::Stmt::Expr(undef_ref.clone(), ir::Expr::Undefined));
            stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), undef_ref));
            stmts.push(ir::Stmt::IfElse(
                test_ref,
                {
                    let alt_ref = ir::Ref::new("alt_");
                    let (mut alt_stmts, alt_value) = convert_expression(*alt, scope);
                    alt_stmts.push(ir::Stmt::Expr(alt_ref.clone(), alt_value));
                    alt_stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), alt_ref));
                    P(ir::Block::with_children(alt_stmts))
                },
                {
                    let cons_ref = ir::Ref::new("cons_");
                    let (mut cons_stmts, cons_value) = convert_expression(*cons, scope);
                    cons_stmts.push(ir::Stmt::Expr(cons_ref.clone(), cons_value));
                    cons_stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), cons_ref));
                    P(ir::Block::with_children(cons_stmts))
                },
            ));
            (stmts, ir::Expr::ReadBinding(value_ref))
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
            let callee_ref = ir::Ref::new("fn_");
            let (mut statements, callee_value) = convert_expr_or_super(callee, scope);
            statements.push(ir::Stmt::Expr(callee_ref.clone(), callee_value));
            let arguments = arguments
                .into_iter()
                .map(|ast::ExprOrSpread { spread, expr }| {
                    let kind = match spread {
                        Some(_) => ir::EleKind::Spread,
                        None => ir::EleKind::Single,
                    };
                    let arg_ref = ir::Ref::new("arg_");
                    let (stmts, arg_value) = convert_expression(*expr, scope);
                    statements.extend(stmts);
                    statements.push(ir::Stmt::Expr(arg_ref.clone(), arg_value));
                    (kind, arg_ref)
                })
                .collect();
            (statements, ir::Expr::Call(call_kind, callee_ref, arguments))
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
                statements.push(ir::Stmt::Expr(ir::Ref::Dead, value));
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

fn convert_variable_declaration(var_decl: ast::VarDecl, scope: &mut ScopeMap) -> Vec<ir::Stmt> {
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
        let init_ref = ir::Ref::new("init_");
        match init {
            Some(init) => {
                let (init_stmts, init_value) = convert_expression(*init, scope);
                stmts.extend(init_stmts);
                stmts.push(ir::Stmt::Expr(init_ref.clone(), init_value));
            }
            None => {
                stmts.push(ir::Stmt::Expr(init_ref.clone(), ir::Expr::Undefined));
            }
        }
        let var_ref = convert_pattern(name, scope);
        stmts.push(ir::Stmt::WriteBinding(var_ref, init_ref));
    }
    stmts
}

fn convert_pattern(pat: ast::Pat, scope: &mut ScopeMap) -> ir::Ref<ir::Mutable> {
    match pat {
        ast::Pat::Ident(ast::Ident {
            sym,
            span: _,
            type_ann: _,
            optional: _,
        }) => {
            let ref_ = ir::Ref::new(sym.clone());
            scope.insert(sym, ref_.clone());
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
    scope: &ScopeMap,
) -> (Vec<ir::Stmt>, ir::Expr) {
    match expr_or_super {
        ast::ExprOrSuper::Expr(expr) => convert_expression(*expr, scope),
        ast::ExprOrSuper::Super(_) => (vec![], ir::Expr::Super),
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
