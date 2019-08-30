use std::collections::HashSet;

use crate::anal::refs;
use crate::ir;

macro_rules! case {
    ( $name:ident, $ir_and_expected:expr ) => {
        #[test]
        fn $name() {
            let (ir, expected): (ir::Block, Vec<ir::Ref<ir::Ssa>>) = $ir_and_expected;
            let refs = refs::used_in_only_one_fn_scope(&ir).collect::<HashSet<_>>();
            assert_eq!(refs, expected.iter().collect())
        }
    };
}

case!(basic, {
    let x = ir::Ref::new("x");
    let y = ir::Ref::new("y");
    let d1 = ir::Ref::dead();
    let d2 = ir::Ref::dead();
    let d3 = ir::Ref::dead();
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x.clone(), expr: ir::Expr::Null },
        ir::Stmt::Expr { target: d1.clone(), expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: y.clone(), expr: ir::Expr::Null },
                ir::Stmt::Expr { target: d2.clone(), expr: ir::Expr::Read { source: y.clone() } },
            ])
        } },
        ir::Stmt::Expr { target: d3.clone(), expr: ir::Expr::Read { source: x.clone() } },
    ]);
    (ir, vec![x, y, d1, d2, d3])
});

case!(basic_bail, {
    let x = ir::Ref::new("x");
    let d1 = ir::Ref::dead();
    let d2 = ir::Ref::dead();
    let d3 = ir::Ref::dead();
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x.clone(), expr: ir::Expr::Null },
        ir::Stmt::Expr { target: d1.clone(), expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: d2.clone(), expr: ir::Expr::Read { source: x.clone() } },
            ])
        } },
        ir::Stmt::Expr { target: d3.clone(), expr: ir::Expr::Read { source: x.clone() } },
    ]);
    (ir, vec![d1, d2, d3])
});

case!(bail_deep, {
    let x = ir::Ref::new("x");
    let d1 = ir::Ref::dead();
    let d2 = ir::Ref::dead();
    let d3 = ir::Ref::dead();
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: x.clone(), expr: ir::Expr::Null },
        ir::Stmt::Expr { target: d1.clone(), expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: d2.clone(), expr: ir::Expr::Function {
                    kind: ir::FnKind::Func { is_async: false, is_generator: false },
                    body: ir::Block(vec![
                        ir::Stmt::Expr { target: d3.clone(), expr: ir::Expr::Read { source: x.clone() } },
                    ])
                } },
            ])
        } },
    ]);
    (ir, vec![d1, d2, d3])
});

case!(time_travel, {
    let x = ir::Ref::new("x");
    let d1 = ir::Ref::dead();
    let d2 = ir::Ref::dead();
    #[rustfmt::skip]
    let ir = ir::Block(vec![
        ir::Stmt::Expr { target: d1.clone(), expr: ir::Expr::Function {
            kind: ir::FnKind::Func { is_async: false, is_generator: false },
            body: ir::Block(vec![
                ir::Stmt::Expr { target: d2.clone(), expr: ir::Expr::Read { source: x.clone() } },
            ])
        } },
        ir::Stmt::Expr { target: x.clone(), expr: ir::Expr::Null },
    ]);
    (ir, vec![d1, d2])
});
