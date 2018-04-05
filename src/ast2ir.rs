use std::iter::once;

use ast;
use ir;
use util::*;

pub fn convert(ast: ast::File) -> ir::Block {
    let ast::File {
        program: ast::Program {
            body,
            directives,
            source_type: _,
        },
    } = ast;

    convert_block(body, directives)
}

fn convert_block(body: Vec<ast::Statement>, directives: Vec<ast::Directive>) -> ir::Block {
    let (children, bindings) = body
        .into_iter()
        .map(convert_statement)
        .fold((vec![], vec![]), |(mut stmts, mut refs), (ss, rs)| {
            stmts.extend(ss);
            refs.extend(rs);
            (stmts, refs)
        });

    ir::Block {
        directives: directives.into_iter().map(|d| d.value.value).collect(),
        bindings,
        children,
    }
}

fn convert_statement(stmt: ast::Statement) -> (Vec<ir::Stmt>, Vec<ir::Ref<ir::Mutable>>) {
    use ast::Statement::*;

    match stmt {
        ExpressionStatement(ast::ExpressionStatement { expression }) => {
            let stmts = convert_expression(expression)
                .coalesce()
                .into_iter()
                .map(ir::Stmt::Expr)
                .collect();
            (stmts, vec![])
        },
        BlockStatement(ast::BlockStatement { body, directives }) =>
            (vec![ir::Stmt::Block(box convert_block(body, directives))], vec![]),
        EmptyStatement(ast::EmptyStatement {}) =>
            (vec![], vec![]),
        DebuggerStatement(ast::DebuggerStatement {}) =>
            (vec![ir::Stmt::Debugger], vec![]),
        WithStatement(_) =>
            unimplemented!("with() statement not supported"),
        ReturnStatement(ast::ReturnStatement { argument }) => {
            let ref_ = ir::Ref::new("return_".to_string());
            let stmts = match argument {
                Some(argument) => {
                    let (exprs, return_value) = convert_expression(argument);
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
            };
            (stmts, vec![])
        },
        LabeledStatement(_) |
        BreakStatement(_) |
        ContinueStatement(_) =>
            unimplemented!("labels not yet supported"),
        IfStatement(ast::IfStatement { test, consequent, alternate }) => {
            let ref_ = ir::Ref::new("if_".to_string());
            let (exprs, test_value) = convert_expression(test);
            let stmts = exprs
                .into_iter()
                .map(ir::Stmt::Expr)
                .chain(once(ir::Stmt::Assign(ref_.clone(), test_value)))
                .chain(once(ir::Stmt::IfElse(
                    ref_,
                    {
                        let (children, bindings) = convert_statement(*consequent);
                        box ir::Block { directives: vec![], bindings, children }
                    },
                    match alternate {
                        Some(alternate) => {
                            let (children, bindings) = convert_statement(*alternate);
                            box ir::Block { directives: vec![], bindings, children }
                        },
                        None => box ir::Block::empty()
                    },
                )))
                .collect();
            (stmts, vec![])
        },
        SwitchStatement(_) =>
            // remember, switch cases aren't evaluated (!) until they're checked for equality
            unimplemented!("switch statements not yet supported"),
        ThrowStatement(ast::ThrowStatement { argument }) => {
            let ref_ = ir::Ref::new("throw_".to_string());
            let (exprs, throw_value) = convert_expression(argument);
            let stmts = exprs
                .into_iter()
                .map(ir::Stmt::Expr)
                .chain(once(ir::Stmt::Assign(ref_.clone(), throw_value)))
                .chain(once(ir::Stmt::Throw(ref_)))
                .collect();
            (stmts, vec![])
        },
        TryStatement(ast::TryStatement { block, handler, finalizer }) => {
            let try = ir::Stmt::Try(
                {
                    let ast::BlockStatement { body, directives } = block;
                    box convert_block(body, directives)
                },
                handler.map(|ast::CatchClause { param, body }| {
                    let ast::BlockStatement { body, directives } = body;
                    (convert_pattern(param), box convert_block(body, directives))
                }),
                finalizer.map(|finalizer| {
                    let ast::BlockStatement { body, directives } = finalizer;
                    box convert_block(body, directives)
                }),
            );
            (vec![try], vec![])
        },
    }
}

fn convert_expression(expr: ast::Expression) -> (Vec<ir::Expr>, ir::Expr) {
    use ast::Expression::*;

    match expr {
        // todo Identifier(ast::Identifier { name }) =>
        RegExpLiteral(ast::RegExpLiteral { pattern, flags }) =>
            (vec![], ir::Expr::RegExp(pattern, flags)),
        NullLiteral(ast::NullLiteral {}) =>
            (vec![], ir::Expr::Null),
        StringLiteral(ast::StringLiteral { value }) =>
            (vec![], ir::Expr::String(value)),
        BooleanLiteral(ast::BooleanLiteral { value }) =>
            (vec![], ir::Expr::Bool(value)),
        NumericLiteral(ast::NumericLiteral { value }) =>
            (vec![], ir::Expr::Bool(value)),
        ThisExpression(ast::ThisExpression {}) =>
            (vec![], ir::Expr::This),
    }
}

fn convert_pattern(pat: ast::Pattern) -> ir::Ref<ir::Mutable> {
    use ast::Pattern::*;

    match pat {
        Identifier(ast::Identifier { name }) =>
            ir::Ref::new(name),
        MemberExpression(_) |
        ObjectPattern(_) |
        ArrayPattern(_) |
        RestElement(_) |
        AssignmentPattern(_) =>
            unimplemented!("complex patterns not yet supported"),
    }
}
