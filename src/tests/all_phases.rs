use crate::ast2ir;
use crate::emit;
use crate::err::NiceError;
use crate::ir2ast;
use crate::opt;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ir = opt::run_passes(g, ir);
                let ast = ir2ast::convert(
                    g,
                    ir,
                    ir2ast::Opt {
                        inline: true,
                        minify: false,
                    },
                );
                let js = emit::emit(g, ast, files, emit::Opt { minify: false })?;
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
(function f() {
    for(;;){
    }
    var _val = y.bar;
    var _obj = z;
    var _val$1;
    if (_val) {
        _val$1 = true;
    } else {
        _val$1 = 'hi';
    }
    _obj.foo = _val$1;
    var _wri = g + 1;
    g = _wri;
    return +[1, {
            x: _val
        }, f + 1, _wri];
})(1);
"###);

case!(
    assign_to_expr,
    r#"
    e |= 0;
    foo().x |= 1;
"#,
@r###"
e = e | 0;
var _obj = foo();
_obj.x = _obj.x | 1;
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
"#,
@r###"
if (foo) {
    just_read_global_state;
}
log(1);
var y = 1;
if (foo) {
    if (bar) {
        y = 10;
    }
}
log(y);
"###);

case!(
    snudown_js_like,
    r#"
    var r;
    something;
    r || (r = {});
    var s = {};
    var o;
    for (o in r) s[o] = r[o];
    r.x = 1;
    for (o in s) r[o] = s[o];
    var stuff = (function(r_inner) {
        return {
            xy: r_inner.x * 2
        };
    })(r);
    var xy = stuff.xy;
    window.foo = function foo(z) {
        return z + xy;
    };
"#,
@r###"
something;
window.foo = function(z) {
    return z + 2;
};
"###);

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
"#,
@r###"
var _fun = function() {
    something();
};
g = foo();
_fun();
_fun();
"###);

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
"#,
@r###"
if (foo) {
    log(g);
}
var y = h;
if (bar()) {
    log(y);
}
"###);

case!(
    dont_inline_into_loop,
    r#"
    let x = g;
    do {
        log(x);
        g = 1;
    } while (foo);
"#,
@r###"
var x = g;
for(;;){
    log(x);
    g = 1;
    if (foo) {
    } else {
        break;
    }
}
"###);

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
"#,
@r###"
if (foo) {
    log(2);
} else {
    log(1);
}
"###);

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
"#,
@r###"
g1 = NaN;
var NaN$1 = 1;
if (foo) {
    NaN$1 = 2;
}
g3 = NaN$1;
"###);

case!(
    referencing_outer_scope_moved_later,
    r#"
    var x; // converted to ssa, moved down to x = 0
    g = function() {
        x();
    };
    x = foo;
"#,
@r###"
g = function() {
    x();
};
var x = foo;
"###);

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
"#,
@r###"
g = function() {
    x();
};
var x = foo;
g2 = function() {
    x = 1;
};
"###);

case!(
    mutually_recursive_fns,
    r#"
    function a() { b(); }
    function b() { c(); }
    function c() { a(); }
    g1 = a;
    g2 = b;
    g3 = c;
"#,
@r###"
var _fun = function() {
    _fun$1();
};
var _fun$1 = function() {
    _fun$2();
};
var _fun$2 = function() {
    _fun();
};
g1 = _fun;
g2 = _fun$1;
g3 = _fun$2;
"###);

case!(
    fn_hoisting_toplevel,
    r#"
    foo();
    function foo() { foo_; }

    (function() {
        bar();
        function bar() { bar_; }
    })();
"#,
@r###"
foo_;
bar_;
"###);

case!(
    fn_hoisting_blocks,
    r#"
    if (x) {
        foo();
        function foo() { foo_; }
    }
    foo();
"#,
@r###"
var foo;
if (x) {
    void 0();
    foo = function() {
        foo_;
    };
}
foo();
"###);

case!(
    fn_hoisting_labelled,
    r#"
    foo();
    label:
    function foo() { foo_; }
"#,
@r###"
var foo;
label: {
    foo = function() {
        foo_;
    };
}
foo();
"###);

case!(
    switch,
    r#"
    switch (x) {
        case 1:
            one;
            break;
        case "foo":
        case bar:
            two;
        default:
            def;
    }
"#,
@r###"
switch(x){
    case 1:
        one;
        break;
    case 'foo':
    case bar: two;
    default: def;
}
"###);

case!(
    switch_scoping_forwards,
    r#"
    switch (x) {
        case 1:
            var v = 2;
            let l = 3;
        default:
            g1 = v;
            g2 = l;
    }
"#,
@r###"
var v;
switch(x){
    case 1:
        v = 2;
        var l = 3;
    default:
        g1 = v;
        g2 = l;
}
"###);

case!(
    switch_scoping_forwards_safe,
    r#"
    switch (x) {
        case 1:
            var v = 2;
            let l = 3;
            g1 = v;
            g2 = l;
        default:
            def;
    }
"#,
@r###"
switch(x){
    case 1:
        g1 = 2;
        g2 = 3;
    default: def;
}
"###);

case!(
    switch_scoping_backwards,
    r#"
    switch (x) {
        case 1:
            g1 = v;
            g2 = l;
            break;
        default:
            var v = 2;
            let l = 3;
    }
"#,
@r###"
var v;
switch(x){
    case 1:
        g1 = v;
        g2 = l;
        break;
    default:
        v = 2;
        var l = 3;
}
"###);

case!(
    switch_dont_forward_past_cases,
    r#"
    switch (x) {
        case 1:
            let y = foo();
        default:
            g = y;
    }
"#,
@r###"
switch(x){
    case 1: var y = foo();
    default: g = y;
}
"###);

case!(
    preserves_prop_calls,
    r#"
    console.log.bind(console);
"#,
@"console.log.bind(console);
");

case!(
    inserts_parens_where_necessary,
    r#"
    g = (x + 1) * 2;
    (function f() {
        f();
    })();
"#,
@r###"
g = (x + 1) * 2;
(function f() {
    f();
})();
"###);
