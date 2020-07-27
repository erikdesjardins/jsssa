use crate::anal::fns;
use crate::ir;

macro_rules! case {
    ( $name:ident, $ir_and_expected:expr ) => {
        #[test]
        fn $name() {
            let (ir, expected): (ir::Block, Vec<ir::Ref<ir::Ssa>>) = $ir_and_expected;
            let refs = fns::without_this(&ir);
            assert_eq!(refs, expected.iter().collect())
        }
    };
}

case!(basic, {
    let x = ir::Ref::new("x");
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x.clone(), expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: ir::Ref::dead(), expr: ir::Expr::Null },
            ])
        } },
    ]);
    (ir, vec![x])
});

case!(basic_bail, {
    let x = ir::Ref::new("x");
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x, expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: ir::Ref::dead(), expr: ir::Expr::This },
            ])
        } },
    ]);
    (ir, vec![])
});

case!(bail_containing_arrow_this, {
    let x = ir::Ref::new("x");
    let y = ir::Ref::new("y");
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x, expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: y, expr: ir::Expr::Function {
                    kind: ir::FnKind::Arrow { is_async: false },
                    body: ir::Block(vec![
                        ir::Stmt::Expr { target: ir::Ref::dead(), expr: ir::Expr::This },
                    ])
                } }
            ])
        } },
    ]);
    (ir, vec![])
});

case!(partial_bail_containing_arrow_this, {
    let x = ir::Ref::new("x");
    let y = ir::Ref::new("y");
    let z = ir::Ref::new("z");
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x.clone(), expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: y, expr: ir::Expr::Function {
                    kind: ir::FnKind::Func { is_async: false, is_generator: false },
                    body: ir::Block(vec![
                        ir::Stmt::Expr { target: z, expr: ir::Expr::Function {
                            kind: ir::FnKind::Arrow { is_async: false },
                            body: ir::Block(vec![
                                ir::Stmt::Expr { target: ir::Ref::dead(), expr: ir::Expr::This },
                            ])
                        } }
                    ])
                } },
            ])
        } },
    ]);
    (ir, vec![x])
});

case!(bail_containing_arrow_this_deep, {
    let x = ir::Ref::new("x");
    let y = ir::Ref::new("y");
    let z = ir::Ref::new("z");
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x, expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: y, expr: ir::Expr::Function {
                    kind: ir::FnKind::Arrow { is_async: false },
                    body: ir::Block(vec![
                        ir::Stmt::Expr { target: z, expr: ir::Expr::Function {
                            kind: ir::FnKind::Arrow { is_async: false },
                            body: ir::Block(vec![
                                ir::Stmt::Expr { target: ir::Ref::dead(), expr: ir::Expr::This },
                            ])
                        } }
                    ])
                } },
            ])
        } },
    ]);
    (ir, vec![])
});
