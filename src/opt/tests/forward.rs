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

        let ir = opt::OptCx::new(ir)
            .run::<forward::Reads>("forward-reads")
            .into_inner();
        let ppr = ir::print(g, &ir);
        insta::assert_snapshot_matches!("basic", ppr);
    })
}

#[test]
fn move_down() {
    swc_globals::with(|g| {
        #[rustfmt::skip]
            let ir = {
            let a = ir::Ref::new("a");
            let b = ir::Ref::new("b");
            // imagine this is a downleveled mut ref:
            // then we want it to declare the value itself, and remove the initializer, for two reasons:
            // 1: better naming
            //   if we have:
            //     _ini = 1
            //     var_name = _ini
            //     <dead> = var_name + 1
            //   we prefer:
            //     var_name = 1
            //     <dead> = var_name + 1
            //   instead of:
            //     _ini = 1
            //     <dead> = _ini + 1
            // 2: avoiding pessimization of time-travelling downleveled refs
            //   if we have:
            //     g = function() { var_name };
            //     _ini = 1
            //     var_name = _ini
            //     var_name + 1
            //   we prefer:
            //     g = function() { var_name };
            //     var_name = 1
            //     var_name + 1
            //   instead of:
            //     g = function() { var_name };
            //     _ini = 1
            //     var_name = 1
            //     _ini + 1
            let dwn = ir::Ref::new("downleveled");
            ir::Block(vec![
                ir::Stmt::Expr { target: a.clone(), expr: ir::Expr::Null },
                ir::Stmt::Expr { target: b.clone(), expr: ir::Expr::Read { source: a } },
                ir::Stmt::Expr { target: dwn, expr: ir::Expr::Read { source: b } },
            ])
        };

        let ir = opt::OptCx::new(ir)
            .run::<forward::Reads>("forward-reads")
            .into_inner();
        let ppr = ir::print(g, &ir);
        insta::assert_snapshot_matches!("move_down", ppr);
    })
}

#[test]
fn dont_move_down_past_effects() {
    swc_globals::with(|g| {
        #[rustfmt::skip]
            let ir = {
            let a = ir::Ref::new("a");
            let b = ir::Ref::new("b");
            let dwn = ir::Ref::new("downleveled");
            ir::Block(vec![
                ir::Stmt::Expr { target: a.clone(), expr: ir::Expr::Null },
                ir::Stmt::Break { label: None },
                ir::Stmt::Expr { target: b.clone(), expr: ir::Expr::Read { source: a } },
                ir::Stmt::Expr { target: dwn, expr: ir::Expr::Read { source: b } },
            ])
        };

        let ir = opt::OptCx::new(ir)
            .run::<forward::Reads>("forward-reads")
            .into_inner();
        let ppr = ir::print(g, &ir);
        insta::assert_snapshot_matches!("dont_move_down_past_effects", ppr);
    })
}
