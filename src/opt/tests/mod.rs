macro_rules! case {
    ( $name:ident, |$cx:ident| $cx_expr:expr, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), crate::utils::NiceError> {
            use crate::{ast2ir, ir, opt, parse, swc_globals};
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let $cx = opt::OptContext::new(ir);
                let ir = opt::OptContext::into_inner($cx_expr);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot_matches!(stringify!($name), ppr, $string);
                Ok(())
            })
        }
    };
}

mod dce;
