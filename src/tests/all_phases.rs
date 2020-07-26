use crate::ast2ir;
use crate::emit;
use crate::err::NiceError;
use crate::ir2ast;
use crate::opt;
use crate::opt_ast;
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
                let ast = opt_ast::run(g, ast);
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
    for(;;);
    var _val = y.bar;
    var _obj = z;
    var _val$1;
    if (_val) _val$1 = true;
    else _val$1 = 'hi';
    _obj.foo = _val$1;
    var _wri = g + 1;
    g = _wri;
    return +[
        1,
        {
            x: _val
        },
        f + 1, _wri];
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
outer: for(;;)inner: for(;;){
    if (foo) continue inner;
    if (bar) break outer;
}
"###);

case!(
    nested_no_side_effects,
    r#"
    let x = 1;
    if (foo) {
        g = just_read_global_state;
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
if (foo) g = just_read_global_state;
log(1);
var y = 1;
if (foo) {
    if (bar) y = 10;
}
log(y);
"###);

case!(
    snudown_js_like,
    r#"
    var r;
    g = something;
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
g = something;
window.foo = function(z) {
    return z + 2;
};
"###);

case!(
    snudown_js_like2,
    r#"
    var o, c = {}, s = {};
    for (o in c) c.hasOwnProperty(o) && (s[o] = c[o]);
    var u = console.log.bind(console), b = console.warn.bind(console);
    for (o in s) s.hasOwnProperty(o) && (c[o] = s[o]);
    s = null;
    var k, v, d, h = 0, w = !1;
    k = c.buffer ? c.buffer : new ArrayBuffer(16777216), c.HEAP8 = v = new Int8Array(k), c.HEAP32 = s = new Int32Array(k), c.HEAPU8 = d = new Uint8Array(k), s[2340] = 5252272;
    var m = [], _ = [], p = [], y = [];
    c.preloadedImages = {}, c.preloadedAudios = {}, s = null, s = '\0\0\0\0\0';
    var g = c._default_renderer = k._default_renderer, A = c._free = k._free;
    c._i64Add = k._i64Add, c._i64Subtract = k._i64Subtract;
    var C = c._wiki_renderer = k._wiki_renderer;
    c.establishStackSpace = k.establishStackSpace;
    var S, x = c.stackAlloc = k.stackAlloc, E = c.stackRestore = k.stackRestore, I = c.stackSave = k.stackSave;
    c.dynCall_iii = k.dynCall_iii, c.dynCall_iiii = k.dynCall_iiii, c.asm = k;
    s && (function (r) {
        var e, i = r.length;
        for (e = 0; e < i; ++e) d[8 + e] = r.charCodeAt(e)
    })(s);
"#,
@r###"
console.log.bind(console);
console.warn.bind(console);
var _alt = new ArrayBuffer(16777216);
new Int8Array(_alt);
var _val = new Int32Array(_alt);
var _val$1 = new Uint8Array(_alt);
_val[2340] = 5252272;
_alt._default_renderer;
_alt._free;
_alt._i64Add;
_alt._i64Subtract;
_alt._wiki_renderer;
_alt.establishStackSpace;
_alt.stackAlloc;
_alt.stackRestore;
_alt.stackSave;
_alt.dynCall_iii;
_alt.dynCall_iiii;
var e = 0;
for(;;){
    if (e < 5) ;
    else break;
    var _prp = 8 + e;
    _val$1[_prp] = '\0\0\0\0\0'.charCodeAt(e);
    e = e + 1;
}
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
    i = function() { return x = y = 1; }
"#,
@r###"
if (foo) log(g);
var y = h;
if (bar()) log(y);
i = function() {
    y = 1;
    return 1;
};
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
    if (foo) ;
    else break;
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
if (foo) log(2);
else log(1);
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
if (foo) NaN$1 = 2;
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
    function foo() { foo_(); }

    (function() {
        bar();
        function bar() { bar_(); }
    })();
"#,
@r###"
foo_();
bar_();
"###);

case!(
    fn_hoisting_blocks,
    r#"
    if (x) {
        foo();
        function foo() { foo_(); }
    }
    foo();
"#,
@r###"
var foo;
if (x) {
    void 0();
    foo = function() {
        foo_();
    };
}
foo();
"###);

case!(
    fn_hoisting_labelled,
    r#"
    foo();
    label:
    function foo() { foo_(); }
"#,
@r###"
var foo;
label: foo = function() {
    foo_();
};
foo();
"###);

case!(
    switch,
    r#"
    switch (x) {
        case 1:
            one();
            break;
        case "foo":
        case bar:
            two();
        default:
            def();
    }
"#,
@r###"
var _tst = bar;
switch(x){
    case 1:
        one();
        break;
    case 'foo':
    case _tst:
        two();
    default:
        def();
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
            def();
    }
"#,
@r###"
switch(x){
    case 1:
        g1 = 2;
        g2 = 3;
    default:
        def();
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
    case 1:
        var y = foo();
    default:
        g = y;
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

case!(
    unreferenced_params_before_referenced,
    r#"
    g = function(a, b, c) {
        h = c;
    };
"#,
@r###"
g = function(_, _$1, c) {
    h = c;
};
"###);

case!(
    arg_shadow_fn_name_decl,
    r#"
    function f(f, a) {
        f(a);
    }
    g = f;
"#,
@r###"
g = function(f, a) {
    f(a);
};
"###);

case!(
    arg_shadow_fn_name_expr,
    r#"
    g = function f(f, a) {
        f(a);
    };
"#,
@r###"
g = function(f, a) {
    f(a);
};
"###);
