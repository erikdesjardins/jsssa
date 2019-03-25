use swc_common::Span;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;
use crate::utils::P;

pub fn convert(ir: ir::Block) -> ast::Script {
    // todo perform inlining at this stage? (i.e. scan backwards for all usages)
    let body = convert_block(ir, &scope::Ir::default());
    ast::Script {
        span: span(),
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
        ir::Stmt::Expr { target, expr } => {
            let expr = convert_expr(expr, scope);
            if target.maybe_used() {
                let name = scope.declare_ssa(target);
                ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
                    span: span(),
                    kind: ast::VarDeclKind::Var,
                    decls: vec![ast::VarDeclarator {
                        span: span(),
                        name: ast::Pat::Ident(ast::Ident {
                            span: span(),
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
        ir::Stmt::DeclareMutable { target, val } => {
            let name = scope.declare_mutable(target);
            ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
                span: span(),
                kind: ast::VarDeclKind::Var,
                decls: vec![ast::VarDeclarator {
                    span: span(),
                    name: ast::Pat::Ident(ast::Ident {
                        span: span(),
                        sym: name,
                        type_ann: None,
                        optional: false,
                    }),
                    init: Some(P(read_ssa_to_expr(val, scope))),
                    definite: false,
                }],
                declare: false,
            }))
        }
        ir::Stmt::WriteMutable { target, val } => match scope.get_mutable(&target) {
            Some(existing_name) => ast::Stmt::Expr(P(ast::Expr::Assign(ast::AssignExpr {
                span: span(),
                op: ast::AssignOp::Assign,
                left: ast::PatOrExpr::Pat(P(ast::Pat::Ident(ast::Ident {
                    span: span(),
                    sym: existing_name,
                    type_ann: None,
                    optional: false,
                }))),
                right: P(read_ssa_to_expr(val, scope)),
            }))),
            None => unreachable!("writing to undeclared mutable ref"),
        },
        ir::Stmt::WriteGlobal { target, val } => {
            ast::Stmt::Expr(P(ast::Expr::Assign(ast::AssignExpr {
                span: span(),
                op: ast::AssignOp::Assign,
                left: ast::PatOrExpr::Pat(P(ast::Pat::Ident(ast::Ident {
                    span: span(),
                    sym: target,
                    type_ann: None,
                    optional: false,
                }))),
                right: P(read_ssa_to_expr(val, scope)),
            })))
        }
        ir::Stmt::WriteMember { obj, prop, val } => {
            ast::Stmt::Expr(P(ast::Expr::Assign(ast::AssignExpr {
                span: span(),
                op: ast::AssignOp::Assign,
                left: ast::PatOrExpr::Pat(P(ast::Pat::Expr(P(ast::Expr::Member(
                    ast::MemberExpr {
                        span: span(),
                        obj: ast::ExprOrSuper::Expr(P(read_ssa_to_expr(obj, scope))),
                        prop: P(read_ssa_to_expr(prop, scope)),
                        computed: true,
                    },
                ))))),
                right: P(read_ssa_to_expr(val, scope)),
            })))
        }
        ir::Stmt::Return { val } => ast::Stmt::Return(ast::ReturnStmt {
            span: span(),
            arg: Some(P(read_ssa_to_expr(val, scope))),
        }),
        ir::Stmt::Throw { val } => ast::Stmt::Throw(ast::ThrowStmt {
            span: span(),
            arg: P(read_ssa_to_expr(val, scope)),
        }),
        ir::Stmt::Break => ast::Stmt::Break(ast::BreakStmt {
            span: span(),
            label: None,
        }),
        ir::Stmt::Continue => ast::Stmt::Continue(ast::ContinueStmt {
            span: span(),
            label: None,
        }),
        ir::Stmt::Debugger => ast::Stmt::Debugger(ast::DebuggerStmt { span: span() }),
        ir::Stmt::Block { body } => ast::Stmt::Block(ast::BlockStmt {
            span: span(),
            stmts: convert_block(*body, scope),
        }),
        ir::Stmt::Loop { body } => ast::Stmt::While(ast::WhileStmt {
            span: span(),
            test: P(ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
                span: span(),
                value: true,
            }))),
            body: P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(*body, scope),
            })),
        }),
        ir::Stmt::ForEach {
            kind,
            init,
            mut body,
        } => {
            let mut for_scope = scope.clone();
            let var_name = match body.children.get(0) {
                Some(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Argument { index: 0 },
                }) => {
                    let name = for_scope.declare_ssa(target.clone());
                    // drop this expr
                    body.children.remove(0);
                    name
                }
                _ => for_scope.declare_ssa(ir::Ref::dead()),
            };
            let left = ast::VarDeclOrPat::VarDecl(ast::VarDecl {
                span: span(),
                kind: ast::VarDeclKind::Var,
                decls: vec![ast::VarDeclarator {
                    span: span(),
                    name: ast::Pat::Ident(ast::Ident {
                        span: span(),
                        sym: var_name,
                        type_ann: None,
                        optional: false,
                    }),
                    init: None,
                    definite: false,
                }],
                declare: false,
            });
            let right = P(read_ssa_to_expr(init, scope));
            let body = P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(*body, &for_scope),
            }));
            match kind {
                ir::ForKind::In => ast::Stmt::ForIn(ast::ForInStmt {
                    span: span(),
                    left,
                    right,
                    body,
                }),
                ir::ForKind::Of => ast::Stmt::ForOf(ast::ForOfStmt {
                    span: span(),
                    await_token: None,
                    left,
                    right,
                    body,
                }),
            }
        }
        ir::Stmt::IfElse { cond, cons, alt } => ast::Stmt::If(ast::IfStmt {
            span: span(),
            test: P(read_ssa_to_expr(cond, scope)),
            cons: P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(*cons, scope),
            })),
            alt: Some(P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(*alt, scope),
            }))),
        }),
        ir::Stmt::Try {
            body,
            catch,
            finally,
        } => ast::Stmt::Try(ast::TryStmt {
            span: span(),
            block: ast::BlockStmt {
                span: span(),
                stmts: convert_block(*body, scope),
            },
            handler: catch.map(|mut body| {
                let mut catch_scope = scope.clone();
                let param_name = match body.children.get(0) {
                    Some(ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::Argument { index: 0 },
                    }) => {
                        let name = catch_scope.declare_ssa(target.clone());
                        // drop this expr
                        body.children.remove(0);
                        Some(name)
                    }
                    _ => None,
                };
                ast::CatchClause {
                    span: span(),
                    param: param_name.map(|param_name| {
                        ast::Pat::Ident(ast::Ident {
                            span: span(),
                            sym: param_name,
                            type_ann: None,
                            optional: false,
                        })
                    }),
                    body: ast::BlockStmt {
                        span: span(),
                        stmts: convert_block(*body, &catch_scope),
                    },
                }
            }),
            finalizer: finally.map(|body| ast::BlockStmt {
                span: span(),
                stmts: convert_block(*body, scope),
            }),
        }),
    }
}

fn convert_expr(expr: ir::Expr, scope: &scope::Ir) -> ast::Expr {
    match expr {
        ir::Expr::Bool { value } => ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
            span: span(),
            value,
        })),
        ir::Expr::Number { value } => ast::Expr::Lit(ast::Lit::Num(ast::Number {
            span: span(),
            value,
        })),
        ir::Expr::String { value, has_escape } => ast::Expr::Lit(ast::Lit::Str(ast::Str {
            span: span(),
            value,
            has_escape,
        })),
        ir::Expr::Null => ast::Expr::Lit(ast::Lit::Null(ast::Null { span: span() })),
        ir::Expr::Undefined => ast::Expr::Unary(ast::UnaryExpr {
            span: span(),
            op: ast::UnaryOp::Void,
            arg: P(ast::Expr::Lit(ast::Lit::Num(ast::Number {
                span: span(),
                value: 0.0,
            }))),
        }),
        ir::Expr::This => ast::Expr::This(ast::ThisExpr { span: span() }),
        ir::Expr::Read { source } => read_ssa_to_expr(source, scope),
        ir::Expr::ReadMutable { source } => {
            let name = match scope.get_mutable(&source) {
                Some(name) => name,
                None => unreachable!("reading from undeclared mutable ref"),
            };
            ast::Expr::Ident(ast::Ident {
                span: span(),
                sym: name,
                type_ann: None,
                optional: false,
            })
        }
        ir::Expr::ReadGlobal { source } => ast::Expr::Ident(ast::Ident {
            span: span(),
            sym: source,
            type_ann: None,
            optional: false,
        }),
        ir::Expr::ReadMember { obj, prop } => ast::Expr::Member(ast::MemberExpr {
            span: span(),
            obj: ast::ExprOrSuper::Expr(P(read_ssa_to_expr(obj, scope))),
            prop: P(read_ssa_to_expr(prop, scope)),
            computed: true,
        }),
        ir::Expr::Array { elems } => ast::Expr::Array(ast::ArrayLit {
            span: span(),
            elems: elems
                .into_iter()
                .map(|elem| {
                    elem.map(|(kind, val)| ast::ExprOrSpread {
                        spread: match kind {
                            ir::EleKind::Single => None,
                            ir::EleKind::Spread => Some(span()),
                        },
                        expr: P(read_ssa_to_expr(val, scope)),
                    })
                })
                .collect(),
        }),
        ir::Expr::Object { props } => ast::Expr::Object(ast::ObjectLit {
            span: span(),
            props: props
                .into_iter()
                .map(|(kind, prop, val)| {
                    ast::PropOrSpread::Prop(P(match kind {
                        ir::PropKind::Simple => ast::Prop::KeyValue(ast::KeyValueProp {
                            key: ast::PropName::Computed(P(read_ssa_to_expr(prop, scope))),
                            value: P(read_ssa_to_expr(val, scope)),
                        }),
                        ir::PropKind::Get | ir::PropKind::Set => {
                            unimplemented!("getter and setter props cannot yet be elaborated")
                        }
                    }))
                })
                .collect(),
        }),
        ir::Expr::RegExp {
            regex,
            has_escape,
            flags,
        } => ast::Expr::Lit(ast::Lit::Regex(ast::Regex {
            span: span(),
            exp: ast::Str {
                span: span(),
                value: regex,
                has_escape,
            },
            flags: flags.map(|f| ast::Str {
                span: span(),
                value: f,
                has_escape: false,
            }),
        })),
        ir::Expr::Unary { op, val } => ast::Expr::Unary(ast::UnaryExpr {
            span: span(),
            op: match op {
                ir::UnaryOp::Plus => ast::UnaryOp::Plus,
                ir::UnaryOp::Minus => ast::UnaryOp::Minus,
                ir::UnaryOp::Not => ast::UnaryOp::Bang,
                ir::UnaryOp::Tilde => ast::UnaryOp::Tilde,
                ir::UnaryOp::Typeof => ast::UnaryOp::TypeOf,
                ir::UnaryOp::Void => ast::UnaryOp::Void,
            },
            arg: P(read_ssa_to_expr(val, scope)),
        }),
        ir::Expr::Binary { op, left, right } => ast::Expr::Bin(ast::BinExpr {
            span: span(),
            op: match op {
                ir::BinaryOp::EqEq => ast::BinaryOp::EqEq,
                ir::BinaryOp::NotEq => ast::BinaryOp::NotEq,
                ir::BinaryOp::StrictEq => ast::BinaryOp::EqEqEq,
                ir::BinaryOp::NotStrictEq => ast::BinaryOp::NotEqEq,
                ir::BinaryOp::Lt => ast::BinaryOp::Lt,
                ir::BinaryOp::LtEq => ast::BinaryOp::LtEq,
                ir::BinaryOp::Gt => ast::BinaryOp::Gt,
                ir::BinaryOp::GtEq => ast::BinaryOp::GtEq,
                ir::BinaryOp::ShiftLeft => ast::BinaryOp::LShift,
                ir::BinaryOp::ShiftRight => ast::BinaryOp::RShift,
                ir::BinaryOp::ShiftRightZero => ast::BinaryOp::ZeroFillRShift,
                ir::BinaryOp::Add => ast::BinaryOp::Add,
                ir::BinaryOp::Sub => ast::BinaryOp::Sub,
                ir::BinaryOp::Mul => ast::BinaryOp::Mul,
                ir::BinaryOp::Div => ast::BinaryOp::Div,
                ir::BinaryOp::Mod => ast::BinaryOp::Mod,
                ir::BinaryOp::BitOr => ast::BinaryOp::BitOr,
                ir::BinaryOp::BitXor => ast::BinaryOp::BitXor,
                ir::BinaryOp::BitAnd => ast::BinaryOp::BitAnd,
                ir::BinaryOp::Exp => ast::BinaryOp::Exp,
                ir::BinaryOp::In => ast::BinaryOp::In,
                ir::BinaryOp::Instanceof => ast::BinaryOp::InstanceOf,
            },
            left: P(read_ssa_to_expr(left, scope)),
            right: P(read_ssa_to_expr(right, scope)),
        }),
        ir::Expr::Delete { obj, prop } => ast::Expr::Unary(ast::UnaryExpr {
            span: span(),
            op: ast::UnaryOp::Delete,
            arg: P(ast::Expr::Member(ast::MemberExpr {
                span: span(),
                obj: ast::ExprOrSuper::Expr(P(read_ssa_to_expr(obj, scope))),
                prop: P(read_ssa_to_expr(prop, scope)),
                computed: true,
            })),
        }),
        ir::Expr::Yield { kind, val } => ast::Expr::Yield(ast::YieldExpr {
            span: span(),
            arg: Some(P(read_ssa_to_expr(val, scope))),
            delegate: match kind {
                ir::YieldKind::Single => false,
                ir::YieldKind::Delegate => true,
            },
        }),
        ir::Expr::Await { val } => ast::Expr::Await(ast::AwaitExpr {
            span: span(),
            arg: P(read_ssa_to_expr(val, scope)),
        }),
        ir::Expr::Call { kind, func, args } => {
            let callee = P(read_ssa_to_expr(func, scope));
            let args = args
                .into_iter()
                .map(|(kind, val)| ast::ExprOrSpread {
                    spread: match kind {
                        ir::EleKind::Single => None,
                        ir::EleKind::Spread => Some(span()),
                    },
                    expr: P(read_ssa_to_expr(val, scope)),
                })
                .collect();
            match kind {
                ir::CallKind::Call => ast::Expr::Call(ast::CallExpr {
                    span: span(),
                    callee: ast::ExprOrSuper::Expr(callee),
                    args,
                    type_args: None,
                }),
                ir::CallKind::New => ast::Expr::New(ast::NewExpr {
                    span: span(),
                    callee,
                    args: Some(args),
                    type_args: None,
                }),
            }
        }
        ir::Expr::Function { kind, name, body } => unimplemented!(),
        ir::Expr::CurrentFunction | ir::Expr::Argument { .. } => {
            unreachable!("CurrentFunction and Argument should be removed by convert_stmt")
        }
    }
}

fn read_ssa_to_expr(ssa_ref: ir::Ref<ir::SSA>, scope: &scope::Ir) -> ast::Expr {
    let name = match scope.get_ssa(&ssa_ref) {
        Some(name) => name,
        None => unreachable!("reading from undeclared SSA ref"),
    };
    ast::Expr::Ident(ast::Ident {
        span: span(),
        sym: name,
        type_ann: None,
        optional: false,
    })
}

fn span() -> Span {
    // todo make sourcemaps work by wiring this through from the original AST
    Span::default()
}
