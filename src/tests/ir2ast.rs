use crate::ast2ir;
use crate::emit;
use crate::err::NiceError;
use crate::ir2ast;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ast2 = ir2ast::convert(
                    g,
                    ir,
                    ir2ast::Opt {
                        inline: true,
                        minify: false,
                    },
                );
                let js = emit::emit(g, ast2, files, emit::Opt { minify: false })?;
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

case!(simple_inlining, "let x = y;");

#[test]
fn no_inlining() -> Result<(), NiceError> {
    swc_globals::with(|g| {
        let (ast, files) = parse::parse(g, "let x = y;")?;
        let ir = ast2ir::convert(g, ast);
        let ast2 = ir2ast::convert(
            g,
            ir,
            ir2ast::Opt {
                inline: false,
                minify: false,
            },
        );
        let js = emit::emit(g, ast2, files, emit::Opt { minify: false })?;
        insta::assert_snapshot_matches!("no_inlining", js);
        Ok(())
    })
}

#[test]
fn minify_names() -> Result<(), NiceError> {
    swc_globals::with(|g| {
        let (ast, files) = parse::parse(
            g,
            r#"
            var x = 1;
            if (x) {
                let x = 3;
                log(x);
            } else {
                let x = 7;
                log(x);
            }
            log(x);
        "#,
        )?;
        let ir = ast2ir::convert(g, ast);
        let ast2 = ir2ast::convert(
            g,
            ir,
            ir2ast::Opt {
                inline: true,
                minify: true,
            },
        );
        let js = emit::emit(g, ast2, files, emit::Opt { minify: false })?;
        insta::assert_snapshot_matches!("minify", js);
        Ok(())
    })
}

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
        good;
    } else {}

    try { good; } catch {} finally {}

    try { good; } catch {} finally { good; }
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
    string_has_escape_behavior,
    r#"
    "foo";
    "ba\r";
    "ba\\z";
"#
);

case!(
    regex_has_escape_behavior,
    r#"
    /foo/;
    /bar\./;
    /ba\/z/;
"#
);

case!(
    no_void_0_initializer,
    r#"
    var x;
    let y;
"#
);

case!(
    object_props,
    r#"
    var obj = {
        x: 1,
        ['bad>']: 2,
        0: 7,
    };
    obj.y = 3;
    obj['#bad'] = 4;
    obj.z;
    obj['%bad'];
    delete obj.w;
    delete obj['^bad'];
    obj.some();
    obj['*bad']();
"#
);
