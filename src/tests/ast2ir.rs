use crate::ast2ir;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() {
            swc_globals::with(|g| {
                let ast = parse::parse(g, $string).unwrap();
                let ir = ast2ir::convert(g, ast);
                insta::assert_debug_snapshot_matches!(stringify!($name), ir);
            });
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
