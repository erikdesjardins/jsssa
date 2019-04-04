use crate::ast2ir;
use crate::err::NiceError;
use crate::ir;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot_matches!(stringify!($name), ppr, $string);
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
    objects,
    r#"
    ({ x, y: 1, [z]: 2 })
"#
);

case!(
    deep_scopes,
    r#"
    var x = 1;
    (function() {
        (function() {
            x = 2;
        });
    });
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

case!(
    var_reassignment_2,
    r#"
    var x = 1;
    const y = 1;
    (function() {
        var x = 2;
        const y = 2;
    });
"#
);

case!(
    var_scoping_1,
    r#"
    var x = 1;
    {
        var x = 2;
    }
    x = 3;
    var x = 4;
"#
);

case!(
    var_scoping_2,
    r#"
    var x = 1;
    {
        let x = 2;
        x = 3;
    }
    x = 4;
"#
);

case!(
    var_hoisting_1,
    r#"
    x = 1;
    {
        var x = 2;
    }
    x = 3;
"#
);

case!(
    var_hoisting_2,
    r#"
    x = 1;
    {
        for (var x of 2);
    }
    x = 3;
"#
);

case!(
    for_in_no_var,
    r#"
    var x;
    for (x in 1);
    for (y in 1);
"#
);

case!(
    conditional,
    r#"
    (cond ? true_ : false_);
"#
);

case!(
    logical_op,
    r#"
    (foo || or_else);
    (bar && and_then);
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
