use crate::ast2ir;
use crate::emit;
use crate::ir2ast;
use crate::opt;
use crate::parse;
use crate::swc_globals;
use crate::utils::NiceError;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ir = opt::run_passes(g, ir);
                let ast = ir2ast::convert(g, ir);
                let js = emit::emit(g, ast, files)?;
                insta::assert_snapshot_matches!(stringify!($name), js, $string);
                Ok(())
            })
        }
    };
}

case!(
    basic,
    r#"
    function f(x) {
        while (true);
        x = y.bar;
        z.foo = x ? true : 'hi';
        return +[1 || x, { x }, f + 1, ++g];
    }
    f(1), true;
"#
);

case!(
    assign_to_expr,
    r#"
    e |= 0;
    foo().x |= 1;
"#
);

case!(
    labels,
    r#"
    outer: for (;;) {
        inner: for (;;) {
            if (foo) continue inner;
            if (bar) break outer;
        }
    }
"#
);

case!(
    nested_no_side_effects,
    r#"
    let x = 1;
    if (foo) {
        just_read_global_state;
    }
    log(x);

    let y = 1;
    if (foo) {
        function maybe_change_y() {
            if (bar) y = 10;
        }
        maybe_change_y();
    }
    log(y);
"#
);
