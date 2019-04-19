// https://github.com/swc-project/swc/blob/48b2607b28deb7bad1808214467afcbef019a00a/ecmascript/transforms/src/fixer.rs
// Apache License

#![feature(box_syntax, box_patterns, specialization)]
#![allow(clippy::all)]

use swc_common::{
    util::{map::Map, move_map::MoveMap},
    Fold, FoldWith,
};
use swc_ecma_ast::*;

use crate::factory::ExprFactory;

mod factory;

pub fn fixer() -> Fixer {
    Fixer {
        ctx: Default::default(),
    }
}

pub struct Fixer {
    ctx: Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Default,
    /// Always treated as expr. (But number of comma-separated expression
    /// matters)
    ///
    ///  - `foo((bar, x))` != `foo(bar, x)`
    ///  - `var foo = (bar, x)` != `var foo = bar, x`
    ///  - `[(foo, bar)]` != `[foo, bar]`
    ForcedExpr {
        is_var_decl: bool,
    },
}
impl Default for Context {
    fn default() -> Self {
        Context::Default
    }
}

impl Fold<KeyValuePatProp> for Fixer {
    fn fold(&mut self, node: KeyValuePatProp) -> KeyValuePatProp {
        let old = self.ctx;
        self.ctx = Context::ForcedExpr { is_var_decl: false }.into();
        let key = node.key.fold_with(self);
        self.ctx = old;

        let value = node.value.fold_with(self);

        KeyValuePatProp { key, value, ..node }
    }
}

impl Fold<AssignPatProp> for Fixer {
    fn fold(&mut self, node: AssignPatProp) -> AssignPatProp {
        let key = node.key.fold_children(self);

        let old = self.ctx;
        self.ctx = Context::ForcedExpr { is_var_decl: false }.into();
        let value = node.value.fold_with(self);
        self.ctx = old;

        AssignPatProp { key, value, ..node }
    }
}

impl Fold<VarDeclarator> for Fixer {
    fn fold(&mut self, node: VarDeclarator) -> VarDeclarator {
        let name = node.name.fold_children(self);

        let old = self.ctx;
        self.ctx = Context::ForcedExpr { is_var_decl: true }.into();
        let init = node.init.fold_with(self);
        self.ctx = old;

        VarDeclarator { name, init, ..node }
    }
}

impl Fold<BlockStmtOrExpr> for Fixer {
    fn fold(&mut self, body: BlockStmtOrExpr) -> BlockStmtOrExpr {
        let body = body.fold_children(self);

        match body {
            BlockStmtOrExpr::Expr(box expr @ Expr::Object(..)) => {
                BlockStmtOrExpr::Expr(box expr.wrap_with_paren())
            }

            _ => body,
        }
    }
}

impl Fold<Stmt> for Fixer {
    fn fold(&mut self, stmt: Stmt) -> Stmt {
        let stmt = match stmt {
            Stmt::Expr(expr) => {
                let old = self.ctx;
                self.ctx = Context::Default.into();
                let expr = expr.fold_with(self);
                self.ctx = old;
                Stmt::Expr(expr)
            }
            _ => stmt.fold_children(self),
        };

        let stmt = match stmt {
            Stmt::Expr(expr) => Stmt::Expr(expr.map(handle_expr_stmt)),

            _ => stmt,
        };

        stmt
    }
}

macro_rules! context_fn_args {
    ($T:tt) => {
        impl Fold<$T> for Fixer {
            fn fold(&mut self, node: $T) -> $T {
                let $T {
                    span,
                    callee,
                    args,
                    type_args,
                } = node;

                let old = self.ctx;
                self.ctx = Context::ForcedExpr { is_var_decl: false }.into();
                let args = args.fold_with(self);
                self.ctx = old;

                $T {
                    span,
                    callee: callee.fold_children(self),
                    args,
                    type_args,
                }
            }
        }
    };
}
context_fn_args!(NewExpr);
context_fn_args!(CallExpr);

macro_rules! array {
    ($T:tt) => {
        impl Fold<$T> for Fixer {
            fn fold(&mut self, e: $T) -> $T {
                let old = self.ctx;
                self.ctx = Context::ForcedExpr { is_var_decl: false }.into();
                let elems = e.elems.fold_with(self);
                self.ctx = old;

                $T { elems, ..e }
            }
        }
    };
}
array!(ArrayLit);
// array!(ArrayPat);

impl Fold<KeyValueProp> for Fixer {
    fn fold(&mut self, prop: KeyValueProp) -> KeyValueProp {
        let prop = prop.fold_children(self);

        match *prop.value {
            Expr::Seq(..) => KeyValueProp {
                value: box (*prop.value).wrap_with_paren(),
                ..prop
            },
            _ => return prop,
        }
    }
}

impl Fold<Expr> for Fixer {
    fn fold(&mut self, expr: Expr) -> Expr {
        fn unwrap_expr(mut e: Expr) -> Expr {
            match e {
                Expr::Seq(SeqExpr { ref mut exprs, .. }) if exprs.len() == 1 => {
                    unwrap_expr(*exprs.pop().unwrap())
                }
                Expr::Paren(ParenExpr { expr, .. }) => unwrap_expr(*expr),
                _ => e,
            }
        }
        let expr = expr.fold_children(self);
        let expr = unwrap_expr(expr);

        match expr {
            Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Fn(_)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Assign(_)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Seq(_)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Update(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Unary(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Bin(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Object(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Cond(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::New(NewExpr { args: None, .. })),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Arrow(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Class(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Yield(..)),
                prop,
            })
            | Expr::Member(MemberExpr {
                span,
                computed,
                obj: ExprOrSuper::Expr(obj @ box Expr::Await(..)),
                prop,
            }) => MemberExpr {
                span,
                computed,
                obj: obj.wrap_with_paren().as_obj(),
                prop,
            }
            .into(),

            // Flatten seq expr
            Expr::Seq(SeqExpr { span, exprs }) => {
                let len = exprs
                    .iter()
                    .map(|expr| match **expr {
                        Expr::Seq(SeqExpr { ref exprs, .. }) => exprs.len(),
                        _ => 1,
                    })
                    .sum();

                let exprs_len = exprs.len();
                let expr = if len == exprs_len {
                    let mut exprs = exprs
                        .into_iter()
                        .enumerate()
                        .filter_map(|(i, e)| {
                            let is_last = i + 1 == exprs_len;
                            if is_last {
                                Some(e)
                            } else {
                                ignore_return_value(e)
                            }
                        })
                        .collect::<Vec<_>>();
                    if exprs.len() == 1 {
                        return *exprs.pop().unwrap();
                    }
                    Expr::Seq(SeqExpr { span, exprs })
                } else {
                    let mut buf = Vec::with_capacity(len);
                    for (i, expr) in exprs.into_iter().enumerate() {
                        let is_last = i + 1 == exprs_len;

                        match *expr {
                            Expr::Seq(SeqExpr { exprs, .. }) => {
                                if !is_last {
                                    buf.extend(exprs.into_iter().filter_map(ignore_return_value));
                                } else {
                                    let exprs_len = exprs.len();
                                    for (i, expr) in exprs.into_iter().enumerate() {
                                        let is_last = i + 1 == exprs_len;
                                        if is_last {
                                            buf.push(expr);
                                        } else {
                                            buf.extend(ignore_return_value(expr));
                                        }
                                    }
                                }
                            }
                            _ => buf.push(expr),
                        }

                        if is_last {

                        } else {

                        }
                    }

                    if buf.len() == 1 {
                        return *buf.pop().unwrap();
                    }
                    buf.shrink_to_fit();
                    Expr::Seq(SeqExpr { span, exprs: buf })
                };

                match self.ctx {
                    Context::ForcedExpr { .. } => Expr::Paren(ParenExpr {
                        span,
                        expr: box expr,
                    }),
                    _ => expr,
                }
            }

            Expr::Bin(mut expr) => {
                expr.right = match *expr.right {
                    e @ Expr::Assign(..)
                    | e @ Expr::Seq(..)
                    | e @ Expr::Yield(..)
                    | e @ Expr::Cond(..) => box e.wrap_with_paren(),
                    Expr::Bin(BinExpr { op: op_of_rhs, .. }) => {
                        if op_of_rhs.precedence() < expr.op.precedence() {
                            box expr.right.wrap_with_paren()
                        } else {
                            expr.right
                        }
                    }
                    _ => expr.right,
                };

                match *expr.left {
                    // While simplifying, (1 + x) * Nan becomes `1 + x * Nan`.
                    // But it should be `(1 + x) * Nan`
                    Expr::Bin(BinExpr { op: op_of_lhs, .. }) => {
                        if op_of_lhs.precedence() < expr.op.precedence() {
                            Expr::Bin(BinExpr {
                                left: box expr.left.wrap_with_paren(),
                                ..expr
                            })
                        } else {
                            Expr::Bin(expr)
                        }
                    }

                    e @ Expr::Seq(..)
                    | e @ Expr::Yield(..)
                    | e @ Expr::Cond(..)
                    | e @ Expr::Assign(..) => Expr::Bin(BinExpr {
                        left: box e.wrap_with_paren(),
                        ..expr
                    }),
                    _ => Expr::Bin(expr),
                }
            }

            Expr::Cond(expr) => {
                let test = match *expr.test {
                    e @ Expr::Seq(..)
                    | e @ Expr::Assign(..)
                    | e @ Expr::Cond(..)
                    | e @ Expr::Arrow(..) => box e.wrap_with_paren(),

                    e @ Expr::Object(..) | e @ Expr::Fn(..) | e @ Expr::Class(..) => {
                        if self.ctx == Context::Default {
                            box e.wrap_with_paren()
                        } else {
                            box e
                        }
                    }
                    _ => expr.test,
                };

                let cons = match *expr.cons {
                    e @ Expr::Seq(..) => box e.wrap_with_paren(),
                    _ => expr.cons,
                };

                let alt = match *expr.alt {
                    e @ Expr::Seq(..) => box e.wrap_with_paren(),
                    _ => expr.alt,
                };
                Expr::Cond(CondExpr {
                    test,
                    cons,
                    alt,
                    ..expr
                })
            }

            Expr::Unary(expr) => {
                let arg = match *expr.arg {
                    e @ Expr::Assign(..)
                    | e @ Expr::Bin(..)
                    | e @ Expr::Seq(..)
                    | e @ Expr::Cond(..)
                    | e @ Expr::Arrow(..)
                    | e @ Expr::Yield(..) => box e.wrap_with_paren(),
                    _ => expr.arg,
                };

                Expr::Unary(UnaryExpr { arg, ..expr })
            }

            Expr::Assign(expr) => {
                let right = match *expr.right {
                    // `foo = (bar = baz)` => foo = bar = baz
                    Expr::Assign(AssignExpr {
                        left: PatOrExpr::Pat(box Pat::Ident(..)),
                        ..
                    })
                    | Expr::Assign(AssignExpr {
                        left: PatOrExpr::Expr(box Expr::Ident(..)),
                        ..
                    }) => expr.right,

                    // Handle `foo = bar = init()
                    Expr::Seq(right) => box right.wrap_with_paren(),
                    _ => expr.right,
                };

                Expr::Assign(AssignExpr { right, ..expr })
            }

            // Function expression cannot start with `function`
            Expr::Call(CallExpr {
                span,
                callee: ExprOrSuper::Expr(callee @ box Expr::Fn(_)),
                args,
                type_args,
            }) => match self.ctx {
                Context::ForcedExpr { .. } => Expr::Call(CallExpr {
                    span,
                    callee: callee.as_callee(),
                    args,
                    type_args,
                }),

                _ => Expr::Call(CallExpr {
                    span,
                    callee: callee.wrap_with_paren().as_callee(),
                    args,
                    type_args,
                }),
            },
            Expr::Call(CallExpr {
                span,
                callee: ExprOrSuper::Expr(callee @ box Expr::Assign(_)),
                args,
                type_args,
            }) => Expr::Call(CallExpr {
                span,
                callee: callee.wrap_with_paren().as_callee(),
                args,
                type_args,
            }),
            _ => expr,
        }
    }
}

fn ignore_return_value(expr: Box<Expr>) -> Option<Box<Expr>> {
    match *expr {
        Expr::Ident(..) | Expr::Fn(..) | Expr::Lit(..) => None,
        Expr::Unary(UnaryExpr {
            op: op!("void"),
            arg,
            ..
        }) => ignore_return_value(arg),
        _ => Some(expr),
    }
}

fn handle_expr_stmt(expr: Expr) -> Expr {
    match expr {
        // It's important for arrow pass to work properly.
        Expr::Object(..) | Expr::Class(..) | Expr::Fn(..) => expr.wrap_with_paren(),

        // ({ a } = foo)
        Expr::Assign(AssignExpr {
            span,
            left: PatOrExpr::Pat(left @ box Pat::Object(..)),
            op,
            right,
        }) => AssignExpr {
            span,
            left: PatOrExpr::Pat(left),
            op,
            right,
        }
        .wrap_with_paren(),

        Expr::Seq(SeqExpr { span, exprs }) => {
            debug_assert!(
                exprs.len() != 1,
                "SeqExpr should be unwrapped if exprs.len() == 1, but length is 1"
            );

            let mut i = 0;
            let len = exprs.len();
            Expr::Seq(SeqExpr {
                span,
                exprs: exprs.move_map(|expr| {
                    i += 1;
                    let is_last = len == i;

                    if !is_last {
                        expr.map(handle_expr_stmt)
                    } else {
                        expr
                    }
                }),
            })
        }

        _ => expr,
    }
}
