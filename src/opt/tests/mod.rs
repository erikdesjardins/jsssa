macro_rules! case {
    ( $name:ident, |$cx:ident| $cx_expr:expr, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), crate::err::NiceError> {
            use crate::{ast2ir, ir, opt, parse, swc_globals};
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let $cx = opt::OptCx::new(ir);
                let ir = opt::OptCx::into_inner($cx_expr);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot!(ppr, @ $expected);
                Ok(())
            })
        }
    };
    ( $name:ident, all_passes, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), crate::err::NiceError> {
            use crate::{ast2ir, ir, opt, parse, swc_globals};
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ir = opt::run_passes(g, ir);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot!(ppr, @ $expected);
                Ok(())
            })
        }
    };
}

mod constant;
mod dce;
mod forward;
mod inline;
mod mut2ssa;
mod redundant;
mod redundant_obj;
mod sroa;
mod unroll;
mod writeonly;
