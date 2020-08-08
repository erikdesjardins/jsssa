use crate::ast2ir;
use crate::err::NiceError;
use crate::ir;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                let ir = ast2ir::convert(g, ast);
                let ppr = ir::print(g, &ir);
                insta::assert_snapshot!(ppr, @ $expected);
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
_ini = <void>
f <= _ini
_fun = <function>:
    f$1 = <current function>
    f$2 <= f$1
    x = <argument 0>
    x$1 <= x
    <loop>:
        _whl = true
        <if> _whl:
            <empty>
        <else>:
            <break>
    _obj = <global y>
    _prp = "bar"
    _val = _obj[_prp]
    x$1 <- _val
    <dead> = _val
    _obj$1 = <global z>
    _prp$1 = "foo"
    _tst = *x$1
    _udf = <void>
    _val$1 <= _udf
    <if> _tst:
        _cns = true
        _val$1 <- _cns
    <else>:
        _alt = "hi"
        _val$1 <- _alt
    _val$2 = *_val$1
    _obj$1[_prp$1] <- _val$2
    <dead> = _val$2
    _prd = 1
    _log <= _prd
    <if> _prd:
        <empty>
    <else>:
        _cns = *x$1
        _log <- _cns
    _ele = *_log
    _key = "x"
    _val$3 = *x$1
    _ele$1 = { [_key]: _val$3 }
    _lhs = *f$2
    _rhs = 1
    _ele$2 = _lhs + _rhs
    _rdr = <global g>
    _one = 1
    _wri = _rdr + _one
    <global g> <- _wri
    _ele$3 = _wri
    _una = [_ele, _ele$1, _ele$2, _ele$3]
    _ret = + _una
    <return> _ret
f <- _fun
_fun$1 = *f
_arg = 1
<dead> = _fun$1(_arg)
<dead> = true
"###);

case!(
    object_props,
    r#"
    o.x;
    o[x];
    o.x = 1;
    o[x] = 1;
    delete o.x;
    delete o[x];
"#,
@r###"
_obj = <global o>
_prp = "x"
<dead> = _obj[_prp]
_obj$1 = <global o>
_prp$1 = <global x>
<dead> = _obj$1[_prp$1]
_obj$2 = <global o>
_prp$2 = "x"
_val = 1
_obj$2[_prp$2] <- _val
<dead> = _val
_obj$3 = <global o>
_prp$3 = <global x>
_val$1 = 1
_obj$3[_prp$3] <- _val$1
<dead> = _val$1
_obj$4 = <global o>
_prp$4 = "x"
<dead> = <delete> _obj$4[_prp$4]
_obj$5 = <global o>
_prp$5 = <global x>
<dead> = <delete> _obj$5[_prp$5]
"###);

case!(
    objects,
    r#"
    ({ x, y: 1, [z]: 2 })
"#,
@r###"
_key = "x"
_val = <global x>
_key$1 = "y"
_val$1 = 1
_key$2 = <global z>
_val$2 = 2
<dead> = { [_key]: _val, [_key$1]: _val$1, [_key$2]: _val$2 }
"###);

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
_ini = <void>
x <= _ini
_ini$1 = 1
x <- _ini$1
<dead> = <function>:
    <dead> = <function>:
        _val = 2
        x <- _val
        <dead> = _val
"###);

case!(
    var_reassignment_1,
    r#"
    var x = 1;
    const y = 1;
    {
        var x = 2;
        const y = 2;
    }
"#,
@r###"
_ini = <void>
x <= _ini
_ini$1 = 1
x <- _ini$1
_ini$2 = 1
y <= _ini$2
_ini$3 = 2
x <- _ini$3
_ini$4 = 2
y$1 <= _ini$4
"###);

case!(
    var_reassignment_2,
    r#"
    var x = 1;
    const y = 1;
    (function() {
        var x = 2;
        const y = 2;
    });
"#,
@r###"
_ini = <void>
x <= _ini
_ini$1 = 1
x <- _ini$1
_ini$2 = 1
y <= _ini$2
<dead> = <function>:
    _ini$3 = <void>
    x$1 <= _ini$3
    _ini$4 = 2
    x$1 <- _ini$4
    _ini$5 = 2
    y$1 <= _ini$5
"###);

case!(
    var_scoping_1,
    r#"
    var x = 1;
    {
        var x = 2;
    }
    x = 3;
    var x = 4;
"#,
@r###"
_ini = <void>
x <= _ini
_ini$1 = 1
x <- _ini$1
_ini$2 = 2
x <- _ini$2
_val = 3
x <- _val
<dead> = _val
_ini$3 = 4
x <- _ini$3
"###);

case!(
    var_scoping_2,
    r#"
    var x = 1;
    {
        let x = 2;
        x = 3;
    }
    x = 4;
"#,
@r###"
_ini = <void>
x <= _ini
_ini$1 = 1
x <- _ini$1
_ini$2 = 2
x$1 <= _ini$2
_val = 3
x$1 <- _val
<dead> = _val
_val$1 = 4
x <- _val$1
<dead> = _val$1
"###);

case!(
    var_hoisting_1,
    r#"
    x = 1;
    {
        var x = 2;
    }
    x = 3;
"#,
@r###"
_ini = <void>
x <= _ini
_val = 1
x <- _val
<dead> = _val
_ini$1 = 2
x <- _ini$1
_val$1 = 3
x <- _val$1
<dead> = _val$1
"###);

case!(
    var_hoisting_2,
    r#"
    x = 1;
    {
        for (var x of 2);
    }
    x = 3;
"#,
@r###"
_ini = <void>
x <= _ini
_val = 1
x <- _val
<dead> = _val
_rhs = 2
<foreach of> _rhs:
    _for = <argument 0>
    x <- _for
_val$1 = 3
x <- _val$1
<dead> = _val$1
"###);

case!(
    for_in_no_var,
    r#"
    var x;
    for (x in 1);
    for (y in 1);
"#,
@r###"
_ini = <void>
x <= _ini
_ini$1 = <void>
x <- _ini$1
_rhs = 1
<foreach in> _rhs:
    _for = <argument 0>
    x <- _for
_rhs$1 = 1
<foreach in> _rhs$1:
    _for = <argument 0>
    <global y> <- _for
"###);

case!(
    conditional,
    r#"
    (cond ? true_ : false_);
"#,
@r###"
_tst = <global cond>
_udf = <void>
_val <= _udf
<if> _tst:
    _cns = <global true_>
    _val <- _cns
<else>:
    _alt = <global false_>
    _val <- _alt
<dead> = *_val
"###);

case!(
    logical_op,
    r#"
    (foo || or_else);
    (bar && and_then);
"#,
@r###"
_prd = <global foo>
_log <= _prd
<if> _prd:
    <empty>
<else>:
    _cns = <global or_else>
    _log <- _cns
<dead> = *_log
_prd$1 = <global bar>
_log$1 <= _prd$1
<if> _prd$1:
    _cns = <global and_then>
    _log$1 <- _cns
<else>:
    <empty>
<dead> = *_log$1
"###);

case!(
    assign_to_expr,
    r#"
    e |= 0;
    foo().x |= 1;
"#,
@r###"
_lhs = <global e>
_rhs = 0
_val = _lhs | _rhs
<global e> <- _val
<dead> = _val
_fun = <global foo>
_obj = _fun()
_prp = "x"
_lhs$1 = _obj[_prp]
_rhs$1 = 1
_val$1 = _lhs$1 | _rhs$1
_obj[_prp] <- _val$1
<dead> = _val$1
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
<label outer>:
    <loop>:
        <label inner>:
            <loop>:
                _iff = <global foo>
                <if> _iff:
                    <continue inner>
                <else>:
                    <empty>
                _iff$1 = <global bar>
                <if> _iff$1:
                    <break outer>
                <else>:
                    <empty>
"###);

case!(
    referencing_outer_scope_declared_later,
    r#"
    g = function() {
        x;
        y;
        z;
    };
    var x = 0;
    let y = 1;
    const z = 2;
"#,
@r###"
_ini = <void>
x <= _ini
_val = <function>:
    <dead> = *x
    <dead> = *y
    <dead> = *z
<global g> <- _val
<dead> = _val
_ini$1 = 0
x <- _ini$1
_ini$2 = 1
y <= _ini$2
_ini$3 = 2
z <= _ini$3
"###);

case!(
    referencing_outer_scope_declared_later2,
    r#"
    g = function() {
        x;
        y; // global
        z; // global
    };
    {
        var x = 0;
        let y = 1;
        const z = 2;
    }
"#,
@r###"
_ini = <void>
x <= _ini
_val = <function>:
    <dead> = *x
    <dead> = <global y>
    <dead> = <global z>
<global g> <- _val
<dead> = _val
_ini$1 = 0
x <- _ini$1
_ini$2 = 1
y <= _ini$2
_ini$3 = 2
z <= _ini$3
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
_ini = <void>
a <= _ini
_ini$1 = <void>
b <= _ini$1
_ini$2 = <void>
c <= _ini$2
_fun = <function>:
    a$1 = <current function>
    a$2 <= a$1
    _fun$3 = *b
    <dead> = _fun$3()
a <- _fun
_fun$1 = <function>:
    b$1 = <current function>
    b$2 <= b$1
    _fun$3 = *c
    <dead> = _fun$3()
b <- _fun$1
_fun$2 = <function>:
    c$1 = <current function>
    c$2 <= c$1
    _fun$3 = *a
    <dead> = _fun$3()
c <- _fun$2
_val = *a
<global g1> <- _val
<dead> = _val
_val$1 = *b
<global g2> <- _val$1
<dead> = _val$1
_val$2 = *c
<global g3> <- _val$2
<dead> = _val$2
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
_ini = <void>
foo <= _ini
_fun = <function>:
    foo$1 = <current function>
    foo$2 <= foo$1
    <dead> = <global foo_>
foo <- _fun
_fun$1 = *foo
<dead> = _fun$1()
_fun$2 = <function>:
    _ini$1 = <void>
    bar <= _ini$1
    _fun$3 = <function>:
        bar$1 = <current function>
        bar$2 <= bar$1
        <dead> = <global bar_>
    bar <- _fun$3
    _fun$4 = *bar
    <dead> = _fun$4()
<dead> = _fun$2()
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
_ini = <void>
foo <= _ini
_iff = <global x>
<if> _iff:
    _fun$1 = *foo
    <dead> = _fun$1()
    _fun$2 = <function>:
        foo$1 = <current function>
        foo$2 <= foo$1
        <dead> = <global foo_>
    foo <- _fun$2
<else>:
    <empty>
_fun = *foo
<dead> = _fun()
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
_swi = <global x>
_tst = 1
_tst$1 = "foo"
_tst$2 = <global bar>
<switch> _swi:
    <case> _tst:
    <dead> = <global one>
    <break>
    <case> _tst$1:
    <case> _tst$2:
    <dead> = <global two>
    <default>:
    <dead> = <global def>
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
_ini = <void>
v <= _ini
_swi = <global x>
_tst = 1
<switch> _swi:
    <case> _tst:
    _ini$1 = 2
    v <- _ini$1
    _ini$2 = 3
    l <= _ini$2
    <default>:
    _val = *v
    <global g1> <- _val
    <dead> = _val
    _val$1 = *l
    <global g2> <- _val$1
    <dead> = _val$1
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
_ini = <void>
v <= _ini
_swi = <global x>
_tst = 1
<switch> _swi:
    <case> _tst:
    _val = *v
    <global g1> <- _val
    <dead> = _val
    _val$1 = *l
    <global g2> <- _val$1
    <dead> = _val$1
    <break>
    <default>:
    _ini$1 = 2
    v <- _ini$1
    _ini$2 = 3
    l <= _ini$2
"###);

case!(
    preserves_prop_calls,
    r#"
    console.log.bind(console);
"#,
@r###"
_obj = <global console>
_prp = "log"
_obj$1 = _obj[_prp]
_prp$1 = "bind"
_arg = <global console>
<dead> = _obj$1[_prp$1](_arg)
"###);

case!(
    does_not_preserve_indirect_calls,
    r#"
    (0, console.log)();
"#,
@r###"
<dead> = 0
_obj = <global console>
_prp = "log"
_fun = _obj[_prp]
<dead> = _fun()
"###);

case!(
    does_not_preserve_new,
    r#"
    new console.log();
"#,
@r###"
_obj = <global console>
_prp = "log"
_fun = _obj[_prp]
<dead> = <new> _fun()
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
_ini = <void>
f <= _ini
_fun = <function>:
    f$1 = <current function>
    f$2 <= f$1
    f$3 = <argument 0>
    f$2 <- f$3
    a = <argument 1>
    a$1 <= a
    _fun$1 = *f$2
    _arg = *a$1
    <dead> = _fun$1(_arg)
f <- _fun
_val = *f
<global g> <- _val
<dead> = _val
"###);

case!(
    arg_shadow_fn_name_expr,
    r#"
    g = function f(f, a) {
        f(a);
    };
"#,
@r###"
_val = <function>:
    f = <current function>
    f$1 <= f
    f$2 = <argument 0>
    f$1 <- f$2
    a = <argument 1>
    a$1 <= a
    _fun = *f$1
    _arg = *a$1
    <dead> = _fun(_arg)
<global g> <- _val
<dead> = _val
"###);

case!(
    decr_object,
    r#"
    obj.x--;
"#,
@r###"
_obj = <global obj>
_prp = "x"
_rdr = _obj[_prp]
_one = 1
_wri = _rdr - _one
_obj[_prp] <- _wri
<dead> = _rdr
"###);

case!(
    incr_object,
    r#"
    ++obj.x;
"#,
@r###"
_obj = <global obj>
_prp = "x"
_rdr = _obj[_prp]
_one = 1
_wri = _rdr + _one
_obj[_prp] <- _wri
<dead> = _wri
"###);
