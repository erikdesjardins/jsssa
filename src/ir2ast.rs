use swc_common::Span;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;
use crate::ir::visit::{visitor_fn, RunVisitor};
use crate::swc_globals;
use crate::utils::P;

mod ssa;

pub fn convert(_: &swc_globals::Initialized, ir: ir::Block) -> ast::Script {
    let mut scope = scope::Ir::default();

    visitor_fn(|stmt| match stmt {
        ir::Stmt::Expr {
            target: _,
            expr: ir::Expr::ReadGlobal { source: global },
        }
        | ir::Stmt::WriteGlobal {
            target: global,
            val: _,
        } => scope.register_global(global),
        _ => {}
    })
    .run_visitor(&ir);

    let mut ssa_cache = ssa::Cache::with_inlining_information(&ir);

    let body = convert_block(ir, &scope, &mut ssa_cache);

    ast::Script {
        span: span(),
        body,
        shebang: None,
    }
}

fn convert_block(
    block: ir::Block,
    parent_scope: &scope::Ir,
    ssa_cache: &mut ssa::Cache,
) -> Vec<ast::Stmt> {
    let mut scope = parent_scope.nested();

    let ir::Block(children) = block;

    children
        .into_iter()
        .flat_map(|stmt| convert_stmt(stmt, &mut scope, ssa_cache))
        .collect()
}

fn convert_stmt(
    stmt: ir::Stmt,
    scope: &mut scope::Ir,
    ssa_cache: &mut ssa::Cache,
) -> Option<ast::Stmt> {
    Some(match stmt {
        ir::Stmt::Expr { target, expr } => {
            return write_ssa_to_stmt(target, expr, scope, ssa_cache)
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
                    init: Some(P(read_ssa_to_expr(val, scope, ssa_cache))),
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
                right: P(read_ssa_to_expr(val, scope, ssa_cache)),
            }))),
            None => unreachable!("writing to undeclared mutable ref: {:?}", target),
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
                right: P(read_ssa_to_expr(val, scope, ssa_cache)),
            })))
        }
        ir::Stmt::WriteMember { obj, prop, val } => {
            ast::Stmt::Expr(P(ast::Expr::Assign(ast::AssignExpr {
                span: span(),
                op: ast::AssignOp::Assign,
                left: ast::PatOrExpr::Pat(P(ast::Pat::Expr(P(ast::Expr::Member(
                    ast::MemberExpr {
                        span: span(),
                        obj: ast::ExprOrSuper::Expr(P(read_ssa_to_expr(obj, scope, ssa_cache))),
                        prop: P(read_ssa_to_expr(prop, scope, ssa_cache)),
                        computed: true,
                    },
                ))))),
                right: P(read_ssa_to_expr(val, scope, ssa_cache)),
            })))
        }
        ir::Stmt::Return { val } => ast::Stmt::Return(ast::ReturnStmt {
            span: span(),
            arg: Some(P(read_ssa_to_expr(val, scope, ssa_cache))),
        }),
        ir::Stmt::Throw { val } => ast::Stmt::Throw(ast::ThrowStmt {
            span: span(),
            arg: P(read_ssa_to_expr(val, scope, ssa_cache)),
        }),
        ir::Stmt::Break { label } => ast::Stmt::Break(ast::BreakStmt {
            span: span(),
            label: label.map(|label| match scope.get_label(&label) {
                Some(name) => ast::Ident {
                    span: span(),
                    sym: name,
                    type_ann: None,
                    optional: false,
                },
                None => unreachable!("breaking from undeclared label: {:?}", label),
            }),
        }),
        ir::Stmt::Continue { label } => ast::Stmt::Continue(ast::ContinueStmt {
            span: span(),
            label: label.map(|label| match scope.get_label(&label) {
                Some(name) => ast::Ident {
                    span: span(),
                    sym: name,
                    type_ann: None,
                    optional: false,
                },
                None => unreachable!("continuing from undeclared label: {:?}", label),
            }),
        }),
        ir::Stmt::Debugger => ast::Stmt::Debugger(ast::DebuggerStmt { span: span() }),
        ir::Stmt::Label { label, body } => {
            let mut label_scope = scope.nested();
            let name = label_scope.declare_label(label);
            ast::Stmt::Labeled(ast::LabeledStmt {
                span: span(),
                label: ast::Ident {
                    span: span(),
                    sym: name,
                    type_ann: None,
                    optional: false,
                },
                body: P(ast::Stmt::Block(ast::BlockStmt {
                    span: span(),
                    stmts: convert_block(body, &label_scope, ssa_cache),
                })),
            })
        }
        ir::Stmt::Loop { body } => ast::Stmt::While(ast::WhileStmt {
            span: span(),
            test: P(ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
                span: span(),
                value: true,
            }))),
            body: P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(body, scope, ssa_cache),
            })),
        }),
        ir::Stmt::ForEach {
            kind,
            init,
            mut body,
        } => {
            let mut for_scope = scope.nested();
            let mut var_name = None;
            body.0
                .drain_filter(|stmt| match stmt {
                    ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::Argument { index: 0 },
                    } => {
                        assert!(var_name.is_none(), "ForEach can only have one argument");
                        var_name = Some(for_scope.declare_ssa(target.clone()));
                        true
                    }
                    _ => false,
                })
                .for_each(drop);
            let left = ast::VarDeclOrPat::VarDecl(ast::VarDecl {
                span: span(),
                kind: ast::VarDeclKind::Var,
                decls: vec![ast::VarDeclarator {
                    span: span(),
                    name: ast::Pat::Ident(ast::Ident {
                        span: span(),
                        sym: var_name.unwrap_or_else(|| for_scope.declare_ssa(ir::Ref::dead())),
                        type_ann: None,
                        optional: false,
                    }),
                    init: None,
                    definite: false,
                }],
                declare: false,
            });
            let right = P(read_ssa_to_expr(init, scope, ssa_cache));
            let body = P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(body, &for_scope, ssa_cache),
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
            test: P(read_ssa_to_expr(cond, scope, ssa_cache)),
            cons: P(ast::Stmt::Block(ast::BlockStmt {
                span: span(),
                stmts: convert_block(cons, scope, ssa_cache),
            })),
            alt: if alt.0.is_empty() {
                None
            } else {
                Some(P(ast::Stmt::Block(ast::BlockStmt {
                    span: span(),
                    stmts: convert_block(alt, scope, ssa_cache),
                })))
            },
        }),
        ir::Stmt::Try {
            body,
            mut catch,
            finally,
        } => ast::Stmt::Try(ast::TryStmt {
            span: span(),
            block: ast::BlockStmt {
                span: span(),
                stmts: convert_block(body, scope, ssa_cache),
            },
            handler: if catch.0.is_empty() && !finally.0.is_empty() {
                None
            } else {
                let mut catch_scope = scope.nested();
                let mut param_name = None;
                catch
                    .0
                    .drain_filter(|stmt| match stmt {
                        ir::Stmt::Expr {
                            target,
                            expr: ir::Expr::Argument { index: 0 },
                        } => {
                            assert!(param_name.is_none(), "Catch can only have one argument");
                            param_name = Some(catch_scope.declare_ssa(target.clone()));
                            true
                        }
                        _ => false,
                    })
                    .for_each(drop);
                Some(ast::CatchClause {
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
                        stmts: convert_block(catch, &catch_scope, ssa_cache),
                    },
                })
            },
            finalizer: if finally.0.is_empty() {
                None
            } else {
                Some(ast::BlockStmt {
                    span: span(),
                    stmts: convert_block(*finally, scope, ssa_cache),
                })
            },
        }),
    })
}

fn convert_expr(expr: ir::Expr, scope: &scope::Ir, ssa_cache: &mut ssa::Cache) -> ast::Expr {
    match expr {
        ir::Expr::Bool { value } => ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
            span: span(),
            value,
        })),
        ir::Expr::Number {
            value: ir::F64(value),
        } => ast::Expr::Lit(ast::Lit::Num(ast::Number {
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
        ir::Expr::Read { source } => read_ssa_to_expr(source, scope, ssa_cache),
        ir::Expr::ReadMutable { source } => {
            let name = match scope.get_mutable(&source) {
                Some(name) => name,
                None => unreachable!("reading from undeclared mutable ref: {:?}", source),
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
            obj: ast::ExprOrSuper::Expr(P(read_ssa_to_expr(obj, scope, ssa_cache))),
            prop: P(read_ssa_to_expr(prop, scope, ssa_cache)),
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
                        expr: P(read_ssa_to_expr(val, scope, ssa_cache)),
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
                            key: ast::PropName::Computed(P(read_ssa_to_expr(
                                prop, scope, ssa_cache,
                            ))),
                            value: P(read_ssa_to_expr(val, scope, ssa_cache)),
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
            flags: Some(ast::Str {
                span: span(),
                value: flags,
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
            arg: P(read_ssa_to_expr(val, scope, ssa_cache)),
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
            left: P(read_ssa_to_expr(left, scope, ssa_cache)),
            right: P(read_ssa_to_expr(right, scope, ssa_cache)),
        }),
        ir::Expr::Delete { obj, prop } => ast::Expr::Unary(ast::UnaryExpr {
            span: span(),
            op: ast::UnaryOp::Delete,
            arg: P(ast::Expr::Member(ast::MemberExpr {
                span: span(),
                obj: ast::ExprOrSuper::Expr(P(read_ssa_to_expr(obj, scope, ssa_cache))),
                prop: P(read_ssa_to_expr(prop, scope, ssa_cache)),
                computed: true,
            })),
        }),
        ir::Expr::Yield { kind, val } => ast::Expr::Yield(ast::YieldExpr {
            span: span(),
            arg: Some(P(read_ssa_to_expr(val, scope, ssa_cache))),
            delegate: match kind {
                ir::YieldKind::Single => false,
                ir::YieldKind::Delegate => true,
            },
        }),
        ir::Expr::Await { val } => ast::Expr::Await(ast::AwaitExpr {
            span: span(),
            arg: P(read_ssa_to_expr(val, scope, ssa_cache)),
        }),
        ir::Expr::Call { kind, func, args } => {
            let callee = P(read_ssa_to_expr(func, scope, ssa_cache));
            let args = args
                .into_iter()
                .map(|(kind, val)| ast::ExprOrSpread {
                    spread: match kind {
                        ir::EleKind::Single => None,
                        ir::EleKind::Spread => Some(span()),
                    },
                    expr: P(read_ssa_to_expr(val, scope, ssa_cache)),
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
        ir::Expr::Function { kind, mut body } => {
            let mut fn_scope = scope.nested();

            let mut fn_name = None;
            body.0
                .drain_filter(|stmt| match stmt {
                    ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::CurrentFunction,
                    } => {
                        assert!(
                            fn_name.is_none(),
                            "CurrentFunction can only be referenced once"
                        );
                        fn_name = Some(fn_scope.declare_ssa(target.clone()));
                        true
                    }
                    _ => false,
                })
                .for_each(drop);

            let params = body
                .0
                .drain_filter(|stmt| match stmt {
                    ir::Stmt::Expr {
                        target: _,
                        expr: ir::Expr::Argument { .. },
                    } => true,
                    _ => false,
                })
                .map(|arg_expr| match arg_expr {
                    ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::Argument { index },
                    } => (target, index),
                    _ => unreachable!(),
                })
                .fold(vec![], |mut param_refs, (target, index)| {
                    // fill in gaps left by unreferenced params
                    param_refs.resize_with(index + 1, ir::Ref::dead);
                    param_refs[index] = target;
                    param_refs
                })
                .into_iter()
                .map(|param_ref| {
                    let param_name = fn_scope.declare_ssa(param_ref);
                    ast::Pat::Ident(ast::Ident {
                        span: span(),
                        sym: param_name,
                        type_ann: None,
                        optional: false,
                    })
                })
                .collect();

            let body = ast::BlockStmt {
                span: span(),
                stmts: convert_block(body, &fn_scope, ssa_cache),
            };

            match kind {
                ir::FnKind::Arrow { is_async } => {
                    assert!(fn_name.is_none(), "arrow function cannot be named");
                    ast::Expr::Arrow(ast::ArrowExpr {
                        span: span(),
                        params,
                        body: ast::BlockStmtOrExpr::BlockStmt(body),
                        is_async,
                        is_generator: false,
                        type_params: None,
                        return_type: None,
                    })
                }
                ir::FnKind::Func {
                    is_async,
                    is_generator,
                } => ast::Expr::Fn(ast::FnExpr {
                    ident: fn_name.map(|fn_name| ast::Ident {
                        span: span(),
                        sym: fn_name,
                        type_ann: None,
                        optional: false,
                    }),
                    function: ast::Function {
                        span: span(),
                        params,
                        decorators: vec![],
                        body: Some(body),
                        is_async,
                        is_generator,
                        type_params: None,
                        return_type: None,
                    },
                }),
            }
        }
        ir::Expr::CurrentFunction | ir::Expr::Argument { .. } => {
            unreachable!("CurrentFunction and Argument should be removed by convert_stmt")
        }
    }
}

fn write_ssa_to_stmt(
    ssa_ref: ir::Ref<ir::Ssa>,
    expr: ir::Expr,
    scope: &mut scope::Ir,
    ssa_cache: &mut ssa::Cache,
) -> Option<ast::Stmt> {
    enum What {
        Emit,
        Cache,
        Define,
    }
    let what = match (ssa_ref.used(), &expr) {
        (ir::Used::Never, _) => What::Emit,
        (ir::Used::Once, _) if ssa_cache.can_be_freely_inlined(&ssa_ref) => What::Cache,
        (_, ir::Expr::Bool { .. })
        | (_, ir::Expr::Number { .. })
        | (_, ir::Expr::Null)
        | (_, ir::Expr::Undefined) => What::Cache,
        (ir::Used::Once, ir::Expr::String { .. }) => What::Cache,
        (ir::Used::Mult, ir::Expr::String { value, .. }) if value.len() <= 32 => What::Cache,
        _ => What::Define,
    };
    let expr = convert_expr(expr, scope, ssa_cache);
    match what {
        What::Emit => Some(ast::Stmt::Expr(P(expr))),
        What::Cache => {
            ssa_cache.cache(ssa_ref, expr);
            None
        }
        What::Define => {
            let name = scope.declare_ssa(ssa_ref);
            Some(ast::Stmt::Decl(ast::Decl::Var(ast::VarDecl {
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
            })))
        }
    }
}

fn read_ssa_to_expr(
    ssa_ref: ir::Ref<ir::Ssa>,
    scope: &scope::Ir,
    ssa_cache: &ssa::Cache,
) -> ast::Expr {
    match ssa_cache.get_cached(&ssa_ref) {
        Some(cached_expr) => ast::Expr::clone(cached_expr),
        None => {
            let name = match scope.get_ssa(&ssa_ref) {
                Some(name) => name,
                None => unreachable!("reading from undeclared ssa ref: {:?}", ssa_ref),
            };
            ast::Expr::Ident(ast::Ident {
                span: span(),
                sym: name,
                type_ann: None,
                optional: false,
            })
        }
    }
}

fn span() -> Span {
    // todo make sourcemaps work by wiring this through from the original AST
    Span::default()
}
