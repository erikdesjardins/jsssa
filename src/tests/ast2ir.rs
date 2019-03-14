use crate::parse;
use crate::ast2ir;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() {
            parse::parse($string, |ast|
                insta::assert_debug_snapshot_matches!(stringify!($name), ast.map(ast2ir::convert)));
        }
    }
}

case!(basic, r#"
    function f(x) {
        while (true);
        x = y.bar;
        z.foo = x ? true : 'hi';
        return +[1 || x, { x }, f + 1, ++g];
    }
    f(1), true;
"#);
