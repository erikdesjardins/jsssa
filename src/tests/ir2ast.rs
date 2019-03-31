use crate::ast2ir;
use crate::emit;
use crate::ir2ast;
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
                let ast2 = ir2ast::convert(g, ir);
                let js = emit::emit(g, ast2, files)?;
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
    deconflict,
    r#"
    var x = 1;
    var x$1 = 1;
    (function() {
        var x = 1;
    })
"#
);

case!(
    deconflict_globals,
    r#"
    var x = 1;
    (function() {
        var x = 1;
        x$1;
        x$2;
    })
"#
);

case!(
    deconflict_for_in,
    r#"
    {
        const x;
    }
    for (x in 1);
"#
);

case!(
    empty_blocks,
    r#"
    if (x) {
        log();
    } else {}

    try { log() } catch {} finally {}

    try { log() } catch {} finally { log() }
"#
);
