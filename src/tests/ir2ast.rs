use crate::ast2ir;
use crate::emit;
use crate::err::NiceError;
use crate::ir2ast;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr, @ $expected:literal ) => {
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
                insta::assert_snapshot!(js, @ $expected);
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
"#,
@r###"
var f;
f = function f$1(x) {
    var f$2 = f$1;
    var x$1 = x;
    for(;;){
        if (true) {
        } else {
            break;
        }
    }
    var _val = y.bar;
    x$1 = _val;
    _val;
    var _obj = z;
    var _val$1;
    if (x$1) {
        _val$1 = true;
    } else {
        _val$1 = 'hi';
    }
    var _val$2 = _val$1;
    _obj.foo = _val$2;
    _val$2;
    var _log = 1;
    if (1) {
    } else {
        _log = x$1;
    }
    var _ele = _log;
    var _ele$1 = {
        x: x$1
    };
    var _ele$2 = f$2 + 1;
    var _wri = g + 1;
    g = _wri;
    return +[
        _ele,
        _ele$1,
        _ele$2,
        _wri
    ];
};
f(1);
true;
"###);

case!(simple_inlining, "let x = y;", @"var x = y;
");

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
        insta::assert_snapshot!(js, @r###"
        var _ini = y;
        var x = _ini;
        "###);
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
        insta::assert_snapshot!(js, @r###"
        var a;
        a = 1;
        if (a) {
            var b = 3;
            log(b);
        } else {
            var b = 7;
            log(b);
        }
        log(a);
        "###);
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
"#,
@r###"
var x;
x = 1;
(function() {
    (function() {
        x = 2;
        2;
    });
});
"###);

case!(
    deconflict,
    r#"
    var x = 1;
    var x$1 = 1;
    (function() {
        var x = 1;
    })
"#,
@r###"
var x;
var x$1;
x = 1;
x$1 = 1;
(function() {
    var x$1$1;
    x$1$1 = 1;
});
"###);

case!(
    deconflict_globals,
    r#"
    var x = 1;
    (function() {
        var x = 1;
        x$1;
        x$2;
    })
"#,
@r###"
var x;
x = 1;
(function() {
    var x$1$1;
    x$1$1 = 1;
    x$1;
    x$2;
});
"###);

case!(
    deconflict_for_in,
    r#"
    {
        const x;
    }
    for (x in 1);
"#,
@r###"
var x$1;
for(var _for in 1){
    x = _for;
}
"###);

case!(
    empty_blocks,
    r#"
    if (x) {
        good;
    } else {}

    try { good; } catch {} finally {}

    try { good; } catch {} finally { good; }
"#,
@r###"
if (x) {
    good;
}
try {
    good;
} catch () {
}
try {
    good;
} finally{
    good;
}
"###);

case!(
    labels,
    r#"
    outer: for (;;) {
        inner: for (;;) {
            if (foo) continue inner;
            if (bar) break outer;
        }
    }
"#,
@r###"
outer: {
    for(;;){
        inner: {
            for(;;){
                if (foo) {
                    continue inner;
                }
                if (bar) {
                    break outer;
                }
            }
        }
    }
}
"###);

case!(
    string_has_escape_behavior,
    r#"
    "foo";
    "ba\r";
    "ba\\z";
"#,
@r###"
'foo';
'ba\r';
'ba\\z';
"###);

case!(
    regex_has_escape_behavior,
    r#"
    /foo/;
    /bar\./;
    /ba\/z/;
"#,
@r###"
/foo/;
/bar\./;
/ba\/z/;
"###);

case!(
    no_void_0_initializer,
    r#"
    var x;
    let y;
"#,
@r###"
var x;
x = void 0;
var y;
"###);

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
"#,
@r###"
var obj;
obj = {
    x: 1,
    'bad>': 2,
    0: 7
};
obj.y = 3;
3;
obj['#bad'] = 4;
4;
obj.z;
obj['%bad'];
delete obj.w;
delete obj['^bad'];
obj.some();
obj['*bad']();
"###);
