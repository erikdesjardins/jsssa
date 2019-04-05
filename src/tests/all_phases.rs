use crate::ast2ir;
use crate::emit;
use crate::err::NiceError;
use crate::ir2ast;
use crate::opt;
use crate::parse;
use crate::swc_globals;

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

case!(
    snudown_js_like,
    r#"
    var r;
    something;
    r || (r = { x: 1 });
    var s = {};
    var o;
    for (o in r) s[o] = r[o];
    var stuff = (function(r_inner) {
        return {
            xy: r_inner * 2
        };
    })();
    var xy = stuff.xy;
    window.foo = function foo(z) {
        return z + xy;
    };
"#
);

case!(
    fn_scopes_do_not_deter_ssa_inlining,
    r#"
    let x = foo();
    function f() {
        something();
    }
    g = x;
    f();
    f();
"#
);

case!(
    inline_into_if_but_not_past_effects,
    r#"
    let x = g;
    if (foo) {
        log(x);
    }
    let y = h;
    if (bar()) {
        log(y);
    }
"#
);

case!(
    dont_inline_into_loop,
    r#"
    let x = g;
    do {
        log(x);
        g = 1;
    } while (foo);
"#
);

case!(
    completely_redundant_var,
    r#"
    var x = 0;
    x += 1;
    var n = x;
    if (foo) {
        x += 1;
        log(x);
    } else {
        log(n);
    }
"#
);

case!(
    deconflict_nan,
    r#"
    g1 = 0 / 0;
    {
        let NaN = 1;
        if (foo) {
            NaN = 2;
        }
        g3 = NaN;
    }
"#
);

case!(
    referencing_outer_scope_moved_later,
    r#"
    var x; // converted to ssa, moved down to x = 0
    g = function() {
        x();
    };
    x = foo;
"#
);

case!(
    referencing_outer_scope_moved_later2,
    r#"
    var x; // stays mutable, moved down to x = 0
    g = function() {
        x();
    };
    x = foo;
    g2 = function() {
        x = 1;
    };
"#
);
