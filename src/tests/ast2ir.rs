use crate::ast2ir;
use crate::ir;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() {
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string).unwrap();
                let ir = ast2ir::convert(g, ast);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot_matches!(stringify!($name), ppr);
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

case!(
    object_props,
    r#"
    o.x;
    o[x];
    o.x = 1;
    o[x] = 1;
    delete o.x;
    delete o[x];
"#
);

case!(
    var_reassignment_1,
    r#"
    var x = 1;
    const y = 1;
    {
        var x = 2;
        const y = 2;
    }
"#
);
