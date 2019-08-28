use crate::opt::dce;
use crate::opt::forward;
use crate::opt::mut2ssa;
use crate::opt::unroll;

macro_rules! passes {
    ( $cx:ident ) => {
        $cx.run::<mut2ssa::Mut2Ssa>("mut2ssa")
            .run::<forward::Reads>("forward-reads-redundancy")
            .converge::<dce::Dce>("dce-forwarded-reads")
            .run::<unroll::Loops>("unroll-loops")
    };
}

case!(
    basic_zero,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
<dead> = {  }
"###);

case!(
    basic_one,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
_key = "x"
_val = 1
<dead> = { [_key]: _val }
_for = _key
x <- _for
_fun = <global log>
_arg = *x
<dead> = _fun(_arg)
"###);

case!(
    other_ops_ok,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    g = 1;
    y.x = 2;
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
_key = "x"
_val = 1
<dead> = { [_key]: _val }
_val$1 = 1
<global g> <- _val$1
_obj = <global y>
_prp = "x"
_val$2 = 2
_obj[_prp] <- _val$2
_for = _key
x <- _for
_fun = <global log>
_arg = *x
<dead> = _fun(_arg)
"###);

case!(
    bail_mutate,
    |cx| passes!(cx),
    r#"
    let something = {};
    something.x = 1;
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
something = {  }
_prp = "x"
_val = 1
something[_prp] <- _val
<foreach in> something:
    _for = <argument 0>
    x <- _for
    _fun = <global log>
    _arg = *x
    <dead> = _fun(_arg)
"###);

case!(
    bail_escape,
    |cx| passes!(cx),
    r#"
    let something = {};
    g = something;
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
something = {  }
<global g> <- something
<foreach in> something:
    _for = <argument 0>
    x <- _for
    _fun = <global log>
    _arg = *x
    <dead> = _fun(_arg)
"###);

case!(
    bail_call,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo();
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
something = {  }
_fun = <global foo>
<dead> = _fun()
<foreach in> something:
    _for = <argument 0>
    x <- _for
    _fun$1 = <global log>
    _arg = *x
    <dead> = _fun$1(_arg)
"###);

case!(
    bail_fn_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    g = function() {
        for (var x in something) {
            log(x);
        }
    };
"#,
@r###"
something = {  }
_val = <function>:
    _ini = <void>
    x <= _ini
    <foreach in> something:
        _for = <argument 0>
        x <- _for
        _fun = <global log>
        _arg = *x
        <dead> = _fun(_arg)
<global g> <- _val
"###);

case!(
    bail_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        for (var x in something) {
            log(x);
        }
        something.x = 1;
    }
"#,
@r###"
_ini = <void>
x <= _ini
something = {  }
<loop>:
    <foreach in> something:
        _for = <argument 0>
        x <- _for
        _fun = <global log>
        _arg = *x
        <dead> = _fun(_arg)
    _prp = "x"
    _val = 1
    something[_prp] <- _val
"###);

case!(
    bail_across_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        something.x = 1;
    }
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
something = {  }
<loop>:
    _prp = "x"
    _val = 1
    something[_prp] <- _val
<foreach in> something:
    _for = <argument 0>
    x <- _for
    _fun = <global log>
    _arg = *x
    <dead> = _fun(_arg)
"###);

case!(
    bail_deep_nonlinear_scopes,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        for (;;) {
            something.x = 1;
        }
    }
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
something = {  }
<loop>:
    <loop>:
        _prp = "x"
        _val = 1
        something[_prp] <- _val
<foreach in> something:
    _for = <argument 0>
    x <- _for
    _fun = <global log>
    _arg = *x
    <dead> = _fun(_arg)
"###);

case!(
    across_safe_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        g = log;
    }
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
<dead> = {  }
<loop>:
    _val = <global log>
    <global g> <- _val
"###);

case!(
    into_linear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    if (g) {
        for (var x in something) {
            log(x);
        }
    }
"#,
@r###"
_ini = <void>
x <= _ini
<dead> = {  }
_iff = <global g>
<if> _iff:
    <empty>
<else>:
    <empty>
"###);

case!(
    across_linear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    if (g) {
        g = log;
    }
    for (var x in something) {
        log(x);
    }
"#,
@r###"
_ini = <void>
x <= _ini
<dead> = {  }
_iff = <global g>
<if> _iff:
    _val = <global log>
    <global g> <- _val
<else>:
    <empty>
"###);

case!(
    bail_across_switch_case,
    |cx| passes!(cx),
    r#"
    switch (foo) {
        case 1:
            let something = {};
        default:
            for (var x in something) {
                log(x);
            }
    }
"#,
@r###"
_ini = <void>
x <= _ini
_swi = <global foo>
_tst = 1
<switch> _swi:
    <case> _tst:
    _ini$1 = {  }
    something <= _ini$1
    <default>:
    _rhs = *something
    <foreach in> _rhs:
        _for = <argument 0>
        x <- _for
        _fun = <global log>
        _arg = *x
        <dead> = _fun(_arg)
"###);

case!(
    bail_on_second_usage,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (let x in something) {
        g = x;
    }
    something.x = 1;
    for (let x in something) {
        g = x;
    }
"#,
@r###"
something = {  }
_prp = "x"
_val = 1
something[_prp] <- _val
<foreach in> something:
    _val$1 = <argument 0>
    <global g> <- _val$1
"###);
