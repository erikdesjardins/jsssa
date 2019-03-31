macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), crate::utils::NiceError> {
            use crate::{ast2ir, ir, opt, parse, swc_globals};
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ir = opt::run_passes(g, ir);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot_matches!(stringify!($name), ppr, $string);
                Ok(())
            })
        }
    };
}

case!(
    precompute,
    r#"
    x = 1 + 1 + 1 + 1;
    y = "foo" + " " + "bar";
"#
);
