use crate::ir;
use crate::opt::{self, forward};
use crate::swc_globals;

#[test]
fn basic() {
    swc_globals::with(|g| {
        #[rustfmt::skip]
            let ir = {
            let a = ir::Ref::new("a");
            let b = ir::Ref::new("b");
            let c = ir::Ref::new("c");
            let d = ir::Ref::new("d");
            ir::Block(vec![
                ir::Stmt::Expr { target: a.clone(), expr: ir::Expr::Null },
                ir::Stmt::Expr { target: b.clone(), expr: ir::Expr::Read { source: a } },
                ir::Stmt::Expr { target: c.clone(), expr: ir::Expr::Read { source: b } },
                ir::Stmt::Expr {
                    target: d,
                    expr: ir::Expr::Binary { op: ir::BinaryOp::Add, left: c.clone(), right: c },
                },
            ])
        };

        let ir = opt::OptContext::new(ir)
            .run::<forward::Reads>("forward-reads")
            .into_inner();
        let ppr = ir::print(g, &ir);
        insta::assert_snapshot_matches!(stringify!(basic), ppr);
    })
}
