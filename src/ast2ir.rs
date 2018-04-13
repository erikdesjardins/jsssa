use std::collections::HashMap;

use ast;
use ir;
use util::*;

type ScopeMap = Map<String, ir::Ref<ir::Mutable>>;

pub fn convert(ast: ast::File) -> ir::Block {
    let ast::File {
        program:
            ast::Program {
                body,
                directives,
                source_type: _,
            },
    } = ast;

    convert_block(body, directives, &HashMap::default())
}

fn convert_block(
    body: Vec<ast::Statement>,
    directives: Vec<ast::Directive>,
    parent_scopes: &ScopeMap,
) -> ir::Block {
    let mut scope = parent_scopes.clone();

    let children = body.into_iter()
        .flat_map(|stmt| convert_statement(stmt, &mut scope))
        .collect();

    ir::Block {
        directives: directives.into_iter().map(|d| d.value.value).collect(),
        children,
    }
}

fn convert_statement(stmt: ast::Statement, scope: &mut ScopeMap) -> Vec<ir::Stmt> {
    use ast::Statement::*;

    match stmt {
        ExpressionStatement(ast::ExpressionStatement { expression }) => {
            let (mut stmts, last_expr) = convert_expression(expression, scope);
            stmts.push(ir::Stmt::Expr(last_expr));
            stmts
        },
        BlockStatement(ast::BlockStatement { body, directives }) =>
            vec![ir::Stmt::Block(box convert_block(body, directives, scope))],
        EmptyStatement(ast::EmptyStatement {}) =>
            vec![],
        DebuggerStatement(ast::DebuggerStatement {}) =>
            vec![ir::Stmt::Debugger],
        WithStatement(_) =>
            unimplemented!("with() statement not supported"),
        ReturnStatement(ast::ReturnStatement { argument }) => {
            let ref_ = ir::Ref::new("return_".to_string());
            match argument {
                Some(argument) => {
                    let (mut stmts, return_value) = convert_expression(argument, scope);
                    stmts.push(ir::Stmt::Assign(ref_.clone(), return_value));
                    stmts.push(ir::Stmt::Return(ref_));
                    stmts
                },
                None => {
                    vec![
                        ir::Stmt::Assign(ref_.clone(), ir::Expr::Undefined),
                        ir::Stmt::Return(ref_),
                    ]
                }
            }
        },
        LabeledStatement(_) =>
            unimplemented!("labels not yet supported"),
        BreakStatement(ast::BreakStatement { label: None }) =>
            vec![ir::Stmt::Break],
        BreakStatement(ast::BreakStatement { label: Some(_) }) =>
            unimplemented!("labels not yet supported"),
        ContinueStatement(ast::ContinueStatement { label: None }) =>
            vec![ir::Stmt::Continue],
        ContinueStatement(ast::ContinueStatement { label: Some(_) }) =>
            unimplemented!("labels not yet supported"),
        IfStatement(ast::IfStatement { test, consequent, alternate }) => {
            let ref_ = ir::Ref::new("if_".to_string());
            let (mut stmts, test_value) = convert_expression(test, scope);
            stmts.push(ir::Stmt::Assign(ref_.clone(), test_value));
            stmts.push(ir::Stmt::IfElse(
                ref_,
                {
                    let children = convert_statement(*consequent, scope);
                    box ir::Block { directives: vec![], children }
                },
                match alternate {
                    Some(alternate) => {
                        let children = convert_statement(*alternate, scope);
                        box ir::Block { directives: vec![], children }
                    },
                    None => box ir::Block::empty()
                },
            ));
            stmts
        },
        SwitchStatement(_) =>
            // remember, switch cases aren't evaluated (!) until they're checked for equality
            unimplemented!("switch statements not yet supported"),
        ThrowStatement(ast::ThrowStatement { argument }) => {
            let ref_ = ir::Ref::new("throw_".to_string());
            let (mut stmts, throw_value) = convert_expression(argument, scope);
            stmts.push(ir::Stmt::Assign(ref_.clone(), throw_value));
            stmts.push(ir::Stmt::Throw(ref_));
            stmts
        },
        TryStatement(ast::TryStatement { block, handler, finalizer }) => {
            let try = ir::Stmt::Try(
                {
                    let ast::BlockStatement { body, directives } = block;
                    box convert_block(body, directives, scope)
                },
                handler.map(|ast::CatchClause { param, body }| {
                    let ast::BlockStatement { body, directives } = body;
                    let mut catch_scope = scope.clone();
                    let ref_ = convert_pattern(param, &mut catch_scope);
                    (ref_, box convert_block(body, directives, &catch_scope))
                }),
                finalizer.map(|finalizer| {
                    let ast::BlockStatement { body, directives } = finalizer;
                    box convert_block(body, directives, scope)
                }),
            );
            vec![try]
        },
        _ => unimplemented!(),
    }
}

fn convert_expression(expr: ast::Expression, scope: &ScopeMap) -> (Vec<ir::Stmt>, ir::Expr) {
    use ast::Expression::*;

    match expr {
        Identifier(ast::Identifier { name }) => {
            let expr = match scope.get(&name) {
                Some(ref_) => ir::Expr::ReadBinding(ref_.clone()),
                None => ir::Expr::ReadGlobal(name),
            };
            (vec![], expr)
        }
        RegExpLiteral(ast::RegExpLiteral { pattern, flags }) => {
            (vec![], ir::Expr::RegExp(pattern, flags))
        }
        NullLiteral(ast::NullLiteral {}) => (vec![], ir::Expr::Null),
        StringLiteral(ast::StringLiteral { value }) => (vec![], ir::Expr::String(value)),
        BooleanLiteral(ast::BooleanLiteral { value }) => (vec![], ir::Expr::Bool(value)),
        NumericLiteral(ast::NumericLiteral { value }) => (vec![], ir::Expr::Number(value)),
        ThisExpression(ast::ThisExpression {}) => (vec![], ir::Expr::This),
        ArrowFunctionExpression(ast::ArrowFunctionExpression {
            params,
            body,
            async,
        }) => {
            use ast::BlockStmtOrExpr::*;

            let mut fn_scope = scope.clone();
            let refs = params
                .into_iter()
                .map(|param| convert_pattern(param, &mut fn_scope))
                .collect();
            let body = match *body {
                BlockStatement(block) => {
                    let ast::BlockStatement { body, directives } = block;
                    convert_block(body, directives, &fn_scope)
                }
                Expression(expr) => {
                    let ref_ = ir::Ref::new("arrow_".to_string());
                    let (mut stmts, return_value) = convert_expression(expr, &fn_scope);
                    stmts.push(ir::Stmt::Assign(ref_.clone(), return_value));
                    stmts.push(ir::Stmt::Return(ref_));
                    ir::Block {
                        directives: vec![],
                        children: stmts,
                    }
                }
            };
            let func = ir::Expr::Function(ir::FnKind::Arrow { async }, None, refs, box body);
            (vec![], func)
        }
        YieldExpression(ast::YieldExpression { argument, delegate }) => {
            let ref_ = ir::Ref::new("yield_".to_string());
            let (mut stmts, yield_value) = match argument {
                Some(argument) => convert_expression(*argument, scope),
                None => (vec![], ir::Expr::Undefined),
            };
            stmts.push(ir::Stmt::Assign(ref_.clone(), yield_value));
            let kind = if delegate {
                ir::YieldKind::Delegate
            } else {
                ir::YieldKind::Single
            };
            (stmts, ir::Expr::Yield(kind, ref_))
        }
        AwaitExpression(ast::AwaitExpression { argument }) => {
            let ref_ = ir::Ref::new("await_".to_string());
            let (mut stmts, await_value) = convert_expression(*argument, scope);
            stmts.push(ir::Stmt::Assign(ref_.clone(), await_value));
            (stmts, ir::Expr::Await(ref_))
        }
        ArrayExpression(ast::ArrayExpression { elements }) => {
            use ast::ExprOrSpread::*;

            let mut statements = vec![];
            let elements = elements
                .into_iter()
                .map(|ele| {
                    ele.map(|e| {
                        let (kind, expr) = match e {
                            Expression(e) => (ir::EleKind::Single, e),
                            SpreadElement(ast::SpreadElement { argument: e }) => {
                                (ir::EleKind::Spread, e)
                            }
                        };
                        let ref_ = ir::Ref::new("ele_".to_string());
                        let (stmts, ele_value) = convert_expression(expr, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Assign(ref_.clone(), ele_value));
                        (kind, ref_)
                    })
                })
                .collect();
            (statements, ir::Expr::Array(elements))
        }
        ObjectExpression(ast::ObjectExpression { properties }) => {
            use ast::PropOrMethodOrSpread::*;

            let mut statements = vec![];
            let properties = properties
                .into_iter()
                .map(|prop| match prop {
                    ObjectProperty(ast::ObjectProperty {
                        key,
                        shorthand: _,
                        value,
                    }) => {
                        let ref_key = ir::Ref::new("key_".to_string());
                        let (stmts, key_value) = convert_expression(key, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Assign(ref_key.clone(), key_value));
                        let ref_value = ir::Ref::new("value_".to_string());
                        let (stmts, value_value) = convert_expression(value, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Assign(ref_value.clone(), value_value));
                        (ir::PropKind::Simple, ref_key, ref_value)
                    }
                    ObjectMethod(ast::ObjectMethod {
                        kind,
                        key,
                        params,
                        body: block,
                        generator,
                        async,
                    }) => {
                        use ast::ObjectMethodKind::*;

                        let kind = match kind {
                            Get => ir::PropKind::Get,
                            Set => ir::PropKind::Set,
                            Method => ir::PropKind::Simple,
                        };
                        let ref_key = ir::Ref::new("key_".to_string());
                        let (stmts, key_value) = convert_expression(key, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Assign(ref_key.clone(), key_value));

                        let mut fn_scope = scope.clone();
                        let param_refs = params
                            .into_iter()
                            .map(|param| convert_pattern(param, &mut fn_scope))
                            .collect();
                        let ast::BlockStatement { body, directives } = block;
                        let body = convert_block(body, directives, &fn_scope);
                        let fn_value = ir::Expr::Function(
                            ir::FnKind::Func {
                                async,
                                gen: generator,
                            },
                            None,
                            param_refs,
                            box body,
                        );
                        let ref_value = ir::Ref::new("value_".to_string());
                        statements.push(ir::Stmt::Assign(ref_value.clone(), fn_value));

                        (kind, ref_key, ref_value)
                    }
                    SpreadElement(_) => unimplemented!("object spread not implemented"),
                })
                .collect();
            (statements, ir::Expr::Object(properties))
        }
        FunctionExpression(ast::FunctionExpression {
            id,
            params,
            body,
            generator,
            async,
        }) => {
            let name = id.map(|id| id.name);
            let mut fn_scope = scope.clone();
            let refs = params
                .into_iter()
                .map(|param| convert_pattern(param, &mut fn_scope))
                .collect();
            let recursive_ref = if let Some(ref name) = &name {
                let recursive_ref = ir::Ref::new(name.clone());
                fn_scope.insert(name.clone(), recursive_ref.clone());
                Some(recursive_ref)
            } else {
                None
            };
            let ast::BlockStatement { body, directives } = body;
            let mut block = convert_block(body, directives, &fn_scope);
            if let Some(recursive_ref) = recursive_ref {
                let desugar_ref = ir::Ref::new("curfn_".to_string());
                block.children.insert(
                    0,
                    ir::Stmt::Assign(desugar_ref.clone(), ir::Expr::CurrentFunction),
                );
                block
                    .children
                    .insert(1, ir::Stmt::AssignBinding(recursive_ref, desugar_ref));
            }

            let func = ir::Expr::Function(
                ir::FnKind::Func {
                    async,
                    gen: generator,
                },
                name,
                refs,
                box block,
            );
            (vec![], func)
        }
        UnaryExpression(ast::UnaryExpression {
            operator,
            prefix: _,
            argument,
        }) => {
            let op = match operator {
                ast::UnaryOperator::Plus => ir::UnaryOp::Plus,
                ast::UnaryOperator::Minus => ir::UnaryOp::Minus,
                ast::UnaryOperator::Not => ir::UnaryOp::Not,
                ast::UnaryOperator::Tilde => ir::UnaryOp::Tilde,
                ast::UnaryOperator::Typeof => ir::UnaryOp::Typeof,
                ast::UnaryOperator::Void => ir::UnaryOp::Void,
                // need to preserve member access
                ast::UnaryOperator::Delete => match *argument {
                    MemberExpression(expr) => {
                        let ast::MemberExpression { object, property } = expr;
                        let obj_ref = ir::Ref::new("obj_".to_string());
                        let (mut stmts, obj_value) = convert_expr_or_super(*object, scope);
                        stmts.push(ir::Stmt::Assign(obj_ref.clone(), obj_value));
                        let prop_ref = ir::Ref::new("prop_".to_string());
                        let (prop_stmts, prop_value) = convert_expression(*property, scope);
                        stmts.extend(prop_stmts);
                        stmts.push(ir::Stmt::Assign(prop_ref.clone(), prop_value));
                        return (stmts, ir::Expr::Delete(obj_ref, prop_ref));
                    }
                    _ => unimplemented!("deletion of non-MemberExpression not supported"),
                },
            };
            let ref_ = ir::Ref::new("unary_".to_string());
            let (mut stmts, expr_value) = convert_expression(*argument, scope);
            stmts.push(ir::Stmt::Assign(ref_.clone(), expr_value));
            (stmts, ir::Expr::Unary(op, ref_))
        }
        UpdateExpression(ast::UpdateExpression {
            operator,
            argument,
            prefix,
        }) => {
            let one_ref = ir::Ref::new("one_".to_string());
            let read_ref = ir::Ref::new("read_".to_string());
            let write_ref = ir::Ref::new("write_".to_string());
            let (read, write) = match *argument {
                Identifier(ast::Identifier { name }) => match scope.get(&name) {
                    Some(ref_) => (
                        ir::Expr::ReadBinding(ref_.clone()),
                        ir::Stmt::AssignBinding(ref_.clone(), write_ref.clone()),
                    ),
                    None => (
                        ir::Expr::ReadGlobal(name.clone()),
                        ir::Stmt::AssignGlobal(name, write_ref.clone()),
                    ),
                },
                arg => panic!("unexpected UpdateExpression argument: {:?}", arg),
            };
            let stmts = vec![
                ir::Stmt::Assign(read_ref.clone(), read),
                ir::Stmt::Assign(one_ref.clone(), ir::Expr::Number(1.0)),
                ir::Stmt::Assign(
                    write_ref.clone(),
                    ir::Expr::Binary(ir::BinaryOp::Add, read_ref.clone(), one_ref),
                ),
                write,
            ];
            let value = if prefix { write_ref } else { read_ref };
            (stmts, ir::Expr::Read(value))
        }
        _ => unimplemented!(),
    }
}

fn convert_pattern(pat: ast::Pattern, scope: &mut ScopeMap) -> ir::Ref<ir::Mutable> {
    use ast::Pattern::*;

    match pat {
        Identifier(ast::Identifier { name }) => {
            let ref_ = ir::Ref::new(name.clone());
            scope.insert(name, ref_.clone());
            ref_
        }
        MemberExpression(_) | ObjectPattern(_) | ArrayPattern(_) | RestElement(_)
        | AssignmentPattern(_) => unimplemented!("complex patterns not yet supported"),
    }
}

fn convert_expr_or_super(
    expr_or_super: ast::ExprOrSuper,
    scope: &ScopeMap,
) -> (Vec<ir::Stmt>, ir::Expr) {
    use ast::ExprOrSuper::*;

    match expr_or_super {
        Expression(expr) => convert_expression(expr, scope),
        Super(ast::Super {}) => (vec![], ir::Expr::Super),
    }
}
