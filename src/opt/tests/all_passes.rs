macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), crate::err::NiceError> {
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

case!(
    downleveling,
    r#"
    let x = 1;
    x = 2;
    x = 3;

    let y = 10;
    log(y);
    log(y + 1);
"#
);

case!(
    downleveling_bail,
    r#"
    let x = 1;
    x = 2;
    x = 3;
    if (foo) log(x);

    let y = 10;
    log(y);
    log(y + 1);
    if (bar) (function() {
        y = 5;
    })();
"#
);
