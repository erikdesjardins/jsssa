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

// todo return a single statement?
// (i.e. use a block for multi statements? would enforce no scope leaks, if we switch to a separate pass)
fn convert_statement(stmt: ast::Statement, scope: &mut ScopeMap) -> Vec<ir::Stmt> {
    use ast::Statement::*;

    match stmt {
        ExpressionStatement(ast::ExpressionStatement { expression }) => {
            let (mut stmts, last_expr) = convert_expression(expression, scope);
            stmts.push(ir::Stmt::Expr(ir::Ref::Dead, last_expr));
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
                    stmts.push(ir::Stmt::Expr(ref_.clone(), return_value));
                    stmts.push(ir::Stmt::Return(ref_));
                    stmts
                },
                None => {
                    vec![
                        ir::Stmt::Expr(ref_.clone(), ir::Expr::Undefined),
                        ir::Stmt::Return(ref_),
                    ]
                }
            }
        },
        LabeledStatement(_) =>
            unimplemented!("labels not yet supported"),
        BreakStatement(ast::BreakStatement { label }) => {
            match label {
                Some(_) => unimplemented!("labels not yet supported"),
                None => vec![ir::Stmt::Break],
            }
        }
        ContinueStatement(ast::ContinueStatement { label }) => {
            match label {
                Some(_) => unimplemented!("labels not yet supported"),
                None => vec![ir::Stmt::Continue],
            }
        }
        IfStatement(ast::IfStatement { test, consequent, alternate }) => {
            let ref_ = ir::Ref::new("if_".to_string());
            let (mut stmts, test_value) = convert_expression(test, scope);
            stmts.push(ir::Stmt::Expr(ref_.clone(), test_value));
            stmts.push(ir::Stmt::IfElse(
                ref_,
                {
                    let children = convert_statement(*consequent, &mut scope.clone());
                    box ir::Block::with_children(children)
                },
                match alternate {
                    Some(alternate) => {
                        let children = convert_statement(*alternate, &mut scope.clone());
                        box ir::Block::with_children(children)
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
            stmts.push(ir::Stmt::Expr(ref_.clone(), throw_value));
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
        while_stmt @ WhileStatement(_) | while_stmt @ DoWhileStatement(_) => {
            let (test, body, prefix) = match while_stmt {
                WhileStatement(ast::WhileStatement { test, body }) => (test, body, true),
                DoWhileStatement(ast::DoWhileStatement { body, test }) => (test, body, false),
                _ => unreachable!(),
            };
            let cond_ref = ir::Ref::new("while_".to_string());
            let (mut test_stmts, test_value) = convert_expression(test, scope);
            test_stmts.push(ir::Stmt::Expr(cond_ref.clone(), test_value));
            test_stmts.push(ir::Stmt::IfElse(
                cond_ref,
                box ir::Block::empty(),
                box ir::Block::with_children(vec![ir::Stmt::Break]),
            ));
            let body_stmts = convert_statement(*body, &mut scope.clone());
            let stmts = if prefix {
                test_stmts.into_iter().chain(body_stmts)
            } else {
                body_stmts.into_iter().chain(test_stmts)
            }.collect();
            vec![ir::Stmt::Loop(box ir::Block::with_children(stmts))]
        }
        ForStatement(ast::ForStatement { init, test, update, body }) => {
            use ast::VarDeclOrExpr::*;

            let mut stmts = match init {
                Some(Expression(init_expr)) => {
                    let (mut init_stmts, init_value) = convert_expression(init_expr, scope);
                    init_stmts.push(ir::Stmt::Expr(ir::Ref::Dead, init_value));
                    init_stmts
                }
                Some(VariableDeclaration(var_decl)) => {
                    convert_variable_declaration(var_decl, scope)
                }
                None => vec![],
            };
            stmts.push(ir::Stmt::Loop(box ir::Block::with_children({
                let mut stmts = match test {
                    Some(test) => {
                        let cond_ref = ir::Ref::new("for_".to_string());
                        let (mut test_stmts, test_value) = convert_expression(test, scope);
                        test_stmts.push(ir::Stmt::Expr(cond_ref.clone(), test_value));
                        test_stmts.push(ir::Stmt::IfElse(
                            cond_ref,
                            box ir::Block::empty(),
                            box ir::Block::with_children(vec![ir::Stmt::Break]),
                        ));
                        test_stmts
                    }
                    None => vec![],
                };
                stmts.extend(convert_statement(*body, &mut scope.clone()));
                if let Some(update) = update {
                    let (update_stmts, update_value) = convert_expression(update, scope);
                    stmts.extend(update_stmts);
                    stmts.push(ir::Stmt::Expr(ir::Ref::Dead, update_value));
                }
                stmts
            })));
            vec![ir::Stmt::Block(box ir::Block::with_children(stmts))]
        }
        for_stmt @ ForInStatement(_) | for_stmt @ ForOfStatement(_) => {
            use ast::VarDeclOrExpr::*;

            let (kind, left, right, body) = match for_stmt {
                ForInStatement(ast::ForInStatement { left, right, body }) => (ir::ForKind::In, left, right, body),
                ForOfStatement(ast::ForOfStatement { left, right, body }) => (ir::ForKind::Of, left, right, body),
                _ => unreachable!(),
            };
            let right_ref = ir::Ref::new("right_".to_string());
            let (mut stmts, right_value) = convert_expression(right, scope);
            stmts.push(ir::Stmt::Expr(right_ref.clone(), right_value));
            let mut body_scope = scope.clone();
            let ele_binding = if_chain! {
                // todo we're definitely gonna need to use `kind`
                if let VariableDeclaration(ast::VariableDeclaration { kind: _, declarations }) = left;
                if declarations.len() == 1;
                if let Some(ast::VariableDeclarator { id, init: None }) = declarations.into_iter().next();
                then {
                    convert_pattern(id, &mut body_scope)
                } else {
                    unimplemented!("for in/of statements with complex initializers not supported");
                }
            };
            let body_stmts = convert_statement(*body, &mut body_scope);
            stmts.push(ir::Stmt::For(
                kind,
                ele_binding,
                right_ref,
                box ir::Block::with_children(body_stmts),
            ));
            stmts
        }
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
                    stmts.push(ir::Stmt::Expr(ref_.clone(), return_value));
                    stmts.push(ir::Stmt::Return(ref_));
                    ir::Block::with_children(stmts)
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
            stmts.push(ir::Stmt::Expr(ref_.clone(), yield_value));
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
            stmts.push(ir::Stmt::Expr(ref_.clone(), await_value));
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
                        statements.push(ir::Stmt::Expr(ref_.clone(), ele_value));
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
                        statements.push(ir::Stmt::Expr(ref_key.clone(), key_value));
                        let ref_value = ir::Ref::new("value_".to_string());
                        let (stmts, value_value) = convert_expression(value, scope);
                        statements.extend(stmts);
                        statements.push(ir::Stmt::Expr(ref_value.clone(), value_value));
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
                        statements.push(ir::Stmt::Expr(ref_key.clone(), key_value));

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
                        statements.push(ir::Stmt::Expr(ref_value.clone(), fn_value));

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
                    ir::Stmt::Expr(desugar_ref.clone(), ir::Expr::CurrentFunction),
                );
                block
                    .children
                    .insert(1, ir::Stmt::WriteBinding(recursive_ref, desugar_ref));
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
                        stmts.push(ir::Stmt::Expr(obj_ref.clone(), obj_value));
                        let prop_ref = ir::Ref::new("prop_".to_string());
                        let (prop_stmts, prop_value) = convert_expression(*property, scope);
                        stmts.extend(prop_stmts);
                        stmts.push(ir::Stmt::Expr(prop_ref.clone(), prop_value));
                        return (stmts, ir::Expr::Delete(obj_ref, prop_ref));
                    }
                    _ => unimplemented!("deletion of non-MemberExpression not supported"),
                },
            };
            let ref_ = ir::Ref::new("unary_".to_string());
            let (mut stmts, expr_value) = convert_expression(*argument, scope);
            stmts.push(ir::Stmt::Expr(ref_.clone(), expr_value));
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
                        ir::Stmt::WriteBinding(ref_.clone(), write_ref.clone()),
                    ),
                    None => (
                        ir::Expr::ReadGlobal(name.clone()),
                        ir::Stmt::WriteGlobal(name, write_ref.clone()),
                    ),
                },
                arg => panic!("unexpected UpdateExpression argument: {:?}", arg),
            };
            let op = match operator {
                ast::UpdateOperator::Incr => ir::BinaryOp::Add,
                ast::UpdateOperator::Decr => ir::BinaryOp::Sub,
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
        BinaryExpression(ast::BinaryExpression {
            operator,
            left,
            right,
        }) => {
            let left_ref = ir::Ref::new("left_".to_string());
            let right_ref = ir::Ref::new("right_".to_string());
            let op = match operator {
                ast::BinaryOperator::Eq => ir::BinaryOp::Eq,
                ast::BinaryOperator::NotEq => ir::BinaryOp::NotEq,
                ast::BinaryOperator::StrictEq => ir::BinaryOp::StrictEq,
                ast::BinaryOperator::NotStrictEq => ir::BinaryOp::NotStrictEq,
                ast::BinaryOperator::Lt => ir::BinaryOp::Lt,
                ast::BinaryOperator::Lte => ir::BinaryOp::Lte,
                ast::BinaryOperator::Gt => ir::BinaryOp::Gt,
                ast::BinaryOperator::Gte => ir::BinaryOp::Gte,
                ast::BinaryOperator::ShiftLeft => ir::BinaryOp::ShiftLeft,
                ast::BinaryOperator::ShiftRight => ir::BinaryOp::ShiftRight,
                ast::BinaryOperator::ShiftRightZero => ir::BinaryOp::ShiftRightZero,
                ast::BinaryOperator::Add => ir::BinaryOp::Add,
                ast::BinaryOperator::Sub => ir::BinaryOp::Sub,
                ast::BinaryOperator::Mul => ir::BinaryOp::Mul,
                ast::BinaryOperator::Div => ir::BinaryOp::Div,
                ast::BinaryOperator::Mod => ir::BinaryOp::Mod,
                ast::BinaryOperator::BitOr => ir::BinaryOp::BitOr,
                ast::BinaryOperator::BitXor => ir::BinaryOp::BitXor,
                ast::BinaryOperator::BitAnd => ir::BinaryOp::BitAnd,
                ast::BinaryOperator::In => ir::BinaryOp::In,
                ast::BinaryOperator::Instanceof => ir::BinaryOp::Instanceof,
            };
            let (mut stmts, left_value) = convert_expression(*left, scope);
            stmts.push(ir::Stmt::Expr(left_ref.clone(), left_value));
            let (right_stmts, right_value) = convert_expression(*right, scope);
            stmts.extend(right_stmts);
            stmts.push(ir::Stmt::Expr(right_ref.clone(), right_value));
            (stmts, ir::Expr::Binary(op, left_ref, right_ref))
        }
        AssignmentExpression(ast::AssignmentExpression {
            operator,
            left,
            right,
        }) => {
            let value_ref = ir::Ref::new("val_".to_string());
            let (mut stmts, read_expr, write_stmt) = match *left {
                ast::PatOrExpr::Pattern(pat) => match pat {
                    ast::Pattern::Identifier(ast::Identifier { name }) => match scope.get(&name) {
                        Some(binding) => (
                            vec![],
                            ir::Expr::ReadBinding(binding.clone()),
                            ir::Stmt::WriteBinding(binding.clone(), value_ref.clone()),
                        ),
                        None => (
                            vec![],
                            ir::Expr::ReadGlobal(name.clone()),
                            ir::Stmt::WriteGlobal(name.clone(), value_ref.clone()),
                        ),
                    },
                    ast::Pattern::MemberExpression(ast::MemberExpression { object, property }) => {
                        let obj_ref = ir::Ref::new("obj_".to_string());
                        let prop_ref = ir::Ref::new("prop_".to_string());
                        let (mut stmts, obj_value) = convert_expr_or_super(*object, scope);
                        stmts.push(ir::Stmt::Expr(obj_ref.clone(), obj_value));
                        let (prop_stmts, prop_value) = convert_expression(*property, scope);
                        stmts.extend(prop_stmts);
                        stmts.push(ir::Stmt::Expr(prop_ref.clone(), prop_value));
                        (
                            stmts,
                            ir::Expr::ReadMember(obj_ref.clone(), prop_ref.clone()),
                            ir::Stmt::WriteMember(obj_ref, prop_ref, value_ref.clone()),
                        )
                    }
                    _ => unimplemented!("assigning to complex patterns not yet supported"),
                },
                ast::PatOrExpr::Expression(_) => {
                    unimplemented!("assigning to Expression (impossible?)")
                }
            };
            match operator {
                ast::AssignmentOperator::Eq => {
                    let (right_stmts, right_val) = convert_expression(*right, scope);
                    stmts.extend(right_stmts);
                    stmts.push(ir::Stmt::Expr(value_ref.clone(), right_val));
                    stmts.push(write_stmt);
                }
                op @ ast::AssignmentOperator::AddEq
                | op @ ast::AssignmentOperator::SubEq
                | op @ ast::AssignmentOperator::MulEq
                | op @ ast::AssignmentOperator::DivEq
                | op @ ast::AssignmentOperator::ModEq
                | op @ ast::AssignmentOperator::ShiftLeftEq
                | op @ ast::AssignmentOperator::ShiftRightEq
                | op @ ast::AssignmentOperator::ShiftRightZeroEq
                | op @ ast::AssignmentOperator::BitOrEq
                | op @ ast::AssignmentOperator::BitXorEq
                | op @ ast::AssignmentOperator::BitAndEq => {
                    let left_ref = ir::Ref::new("left_".to_string());
                    let right_ref = ir::Ref::new("right_".to_string());
                    let op = match op {
                        ast::AssignmentOperator::Eq => unreachable!(),
                        ast::AssignmentOperator::AddEq => ir::BinaryOp::Add,
                        ast::AssignmentOperator::SubEq => ir::BinaryOp::Sub,
                        ast::AssignmentOperator::MulEq => ir::BinaryOp::Mul,
                        ast::AssignmentOperator::DivEq => ir::BinaryOp::Div,
                        ast::AssignmentOperator::ModEq => ir::BinaryOp::Mod,
                        ast::AssignmentOperator::ShiftLeftEq => ir::BinaryOp::ShiftLeft,
                        ast::AssignmentOperator::ShiftRightEq => ir::BinaryOp::ShiftRight,
                        ast::AssignmentOperator::ShiftRightZeroEq => ir::BinaryOp::ShiftRightZero,
                        ast::AssignmentOperator::BitOrEq => ir::BinaryOp::BitOr,
                        ast::AssignmentOperator::BitXorEq => ir::BinaryOp::BitXor,
                        ast::AssignmentOperator::BitAndEq => ir::BinaryOp::BitAnd,
                    };
                    stmts.push(ir::Stmt::Expr(left_ref.clone(), read_expr));
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
        LogicalExpression(ast::LogicalExpression {
            operator,
            left,
            right,
        }) => {
            let left_ref = ir::Ref::new("pred_".to_string());
            let value_ref = ir::Ref::new("logi_".to_string());
            let (mut stmts, left_value) = convert_expression(*left, scope);
            stmts.push(ir::Stmt::Expr(left_ref.clone(), left_value));
            stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), left_ref.clone()));
            let (consequent, alternate) = {
                let right_ref = ir::Ref::new("cons_".to_string());
                let (mut right_stmts, right_value) = convert_expression(*right, scope);
                right_stmts.push(ir::Stmt::Expr(right_ref.clone(), right_value));
                right_stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), right_ref));
                let full = ir::Block::with_children(right_stmts);
                let empty = ir::Block::empty();
                match operator {
                    ast::LogicalOperator::Or => (empty, full),
                    ast::LogicalOperator::And => (full, empty),
                }
            };
            stmts.push(ir::Stmt::IfElse(left_ref, box consequent, box alternate));
            (stmts, ir::Expr::ReadBinding(value_ref))
        }
        MemberExpression(ast::MemberExpression { object, property }) => {
            let obj_ref = ir::Ref::new("obj_".to_string());
            let prop_ref = ir::Ref::new("prop_".to_string());
            let (mut stmts, obj_value) = convert_expr_or_super(*object, scope);
            stmts.push(ir::Stmt::Expr(obj_ref.clone(), obj_value));
            let (prop_stmts, prop_value) = convert_expression(*property, scope);
            stmts.extend(prop_stmts);
            stmts.push(ir::Stmt::Expr(prop_ref.clone(), prop_value));
            (stmts, ir::Expr::ReadMember(obj_ref, prop_ref))
        }
        ConditionalExpression(ast::ConditionalExpression {
            test,
            alternate,
            consequent,
        }) => {
            let test_ref = ir::Ref::new("test_".to_string());
            let undef_ref = ir::Ref::new("undef_".to_string());
            let value_ref = ir::Ref::new("value_".to_string());
            let (mut stmts, test_value) = convert_expression(*test, scope);
            stmts.push(ir::Stmt::Expr(test_ref.clone(), test_value));
            stmts.push(ir::Stmt::Expr(undef_ref.clone(), ir::Expr::Undefined));
            stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), undef_ref));
            stmts.push(ir::Stmt::IfElse(
                test_ref,
                {
                    let alt_ref = ir::Ref::new("alt_".to_string());
                    let (mut alt_stmts, alt_value) = convert_expression(*alternate, scope);
                    alt_stmts.push(ir::Stmt::Expr(alt_ref.clone(), alt_value));
                    alt_stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), alt_ref));
                    box ir::Block::with_children(alt_stmts)
                },
                {
                    let cons_ref = ir::Ref::new("cons_".to_string());
                    let (mut cons_stmts, cons_value) = convert_expression(*consequent, scope);
                    cons_stmts.push(ir::Stmt::Expr(cons_ref.clone(), cons_value));
                    cons_stmts.push(ir::Stmt::WriteBinding(value_ref.clone(), cons_ref));
                    box ir::Block::with_children(cons_stmts)
                },
            ));
            (stmts, ir::Expr::ReadBinding(value_ref))
        }
        call_expr @ CallExpression(_) | call_expr @ NewExpression(_) => {
            use ast::ExprOrSpread::*;

            let callee_ref = ir::Ref::new("fn_".to_string());
            let (callee, arguments, call_kind) = match call_expr {
                CallExpression(ast::CallExpression { callee, arguments }) => {
                    (callee, arguments, ir::CallKind::Call)
                }
                NewExpression(ast::NewExpression { callee, arguments }) => {
                    (callee, arguments, ir::CallKind::New)
                }
                _ => unreachable!(),
            };
            let (mut statements, callee_value) = convert_expr_or_super(*callee, scope);
            statements.push(ir::Stmt::Expr(callee_ref.clone(), callee_value));
            let arguments = arguments
                .into_iter()
                .map(|arg| {
                    let (kind, expr) = match arg {
                        Expression(e) => (ir::EleKind::Single, e),
                        SpreadElement(ast::SpreadElement { argument: e }) => {
                            (ir::EleKind::Spread, e)
                        }
                    };
                    let ref_ = ir::Ref::new("arg_".to_string());
                    let (stmts, arg_value) = convert_expression(expr, scope);
                    statements.extend(stmts);
                    statements.push(ir::Stmt::Expr(ref_.clone(), arg_value));
                    (kind, ref_)
                })
                .collect();
            (statements, ir::Expr::Call(call_kind, callee_ref, arguments))
        }
        SequenceExpression(ast::SequenceExpression { expressions }) => {
            let mut expressions: Vec<_> = expressions
                .into_iter()
                .map(|expr| convert_expression(expr, scope))
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
        TemplateLiteral(_) | TaggedTemplateExpression(_) => {
            unimplemented!("templates not yet supported")
        }
        ClassExpression(_) => unimplemented!("classes not yet supported"),
    }
}

fn convert_variable_declaration(
    var_decl: ast::VariableDeclaration,
    scope: &mut ScopeMap,
) -> Vec<ir::Stmt> {
    // todo we're definitely gonna need to handle `kind`
    let ast::VariableDeclaration {
        kind: _,
        declarations,
    } = var_decl;
    let mut stmts = vec![];
    for declarator in declarations.into_iter() {
        let ast::VariableDeclarator { id, init } = declarator;
        let init_ref = ir::Ref::new("init_".to_string());
        match init {
            Some(init) => {
                let (init_stmts, init_value) = convert_expression(init, scope);
                stmts.extend(init_stmts);
                stmts.push(ir::Stmt::Expr(init_ref.clone(), init_value));
            }
            None => {
                stmts.push(ir::Stmt::Expr(init_ref.clone(), ir::Expr::Undefined));
            }
        }
        let var_ref = convert_pattern(id, scope);
        stmts.push(ir::Stmt::WriteBinding(var_ref, init_ref));
    }
    stmts
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
