use std::collections::HashMap;
use std::iter::once;

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
            convert_expression(expression, scope)
                .coalesce()
                .into_iter()
                .map(ir::Stmt::Expr)
                .collect()
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
                    let (exprs, return_value) = convert_expression(argument, scope);
                    exprs
                        .into_iter()
                        .map(ir::Stmt::Expr)
                        .chain(once(ir::Stmt::Assign(ref_.clone(), return_value)))
                        .chain(once(ir::Stmt::Return(ref_)))
                        .collect()
                },
                None => {
                    vec![
                        ir::Stmt::Assign(ref_.clone(), ir::Expr::Undefined),
                        ir::Stmt::Return(ref_),
                    ]
                }
            }
        },
        LabeledStatement(_) |
        BreakStatement(_) |
        ContinueStatement(_) =>
            unimplemented!("labels not yet supported"),
        IfStatement(ast::IfStatement { test, consequent, alternate }) => {
            let ref_ = ir::Ref::new("if_".to_string());
            let (exprs, test_value) = convert_expression(test, scope);
            exprs
                .into_iter()
                .map(ir::Stmt::Expr)
                .chain(once(ir::Stmt::Assign(ref_.clone(), test_value)))
                .chain(once(ir::Stmt::IfElse(
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
                )))
                .collect()
        },
        SwitchStatement(_) =>
            // remember, switch cases aren't evaluated (!) until they're checked for equality
            unimplemented!("switch statements not yet supported"),
        ThrowStatement(ast::ThrowStatement { argument }) => {
            let ref_ = ir::Ref::new("throw_".to_string());
            let (exprs, throw_value) = convert_expression(argument, scope);
            exprs
                .into_iter()
                .map(ir::Stmt::Expr)
                .chain(once(ir::Stmt::Assign(ref_.clone(), throw_value)))
                .chain(once(ir::Stmt::Throw(ref_)))
                .collect()
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

fn convert_expression(expr: ast::Expression, scope: &ScopeMap) -> (Vec<ir::Expr>, ir::Expr) {
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
