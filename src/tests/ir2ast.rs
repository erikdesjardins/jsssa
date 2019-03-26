use crate::ast2ir;
use crate::emit;
use crate::ir2ast;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string).unwrap();
                let ir = ast2ir::convert(g, ast);
                let ast2 = ir2ast::convert(g, ir);
                let js = emit::emit(g, ast2, files).unwrap();
                insta::assert_snapshot_matches!(stringify!($name), js);
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
