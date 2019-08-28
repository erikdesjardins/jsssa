use crate::opt::mut2ssa;

case!(
    basic,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    let x = 1;
    x = 2;
    x = 3;

    let y = 10;
    log(y);
    log(y + 1);
"#,
@r###"
<dead> = 1
_val = 2
<dead> = _val
_val$1 = 3
<dead> = _val$1
_ini = 10
y = _ini
_fun = <global log>
_arg = y
<dead> = _fun(_arg)
_fun$1 = <global log>
_lhs = y
_rhs = 1
_arg$1 = _lhs + _rhs
<dead> = _fun$1(_arg$1)
"###);

case!(
    basic_bail,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    let x = 1;
    x = 2;
    x = 3;
    if (foo) log(x);

    let y = 10;
    log(y);
    log(y + 1);
    if (bar) (function() {
        y = 5;
    })();
"#,
@r###"
_ini = 1
x <= _ini
_val = 2
x <- _val
<dead> = _val
_val$1 = 3
x <- _val$1
<dead> = _val$1
_iff = <global foo>
<if> _iff:
    _fun$2 = <global log>
    _arg$2 = *x
    <dead> = _fun$2(_arg$2)
<else>:
    <empty>
_ini$1 = 10
y <= _ini$1
_fun = <global log>
_arg = *y
<dead> = _fun(_arg)
_fun$1 = <global log>
_lhs = *y
_rhs = 1
_arg$1 = _lhs + _rhs
<dead> = _fun$1(_arg$1)
_iff$1 = <global bar>
<if> _iff$1:
    _fun$2 = <function>:
        _val$2 = 5
        y <- _val$2
        <dead> = _val$2
    <dead> = _fun$2()
<else>:
    <empty>
"###);

case!(
    downleveling,
    all_passes,
    r#"
    let x = 1;
    x = 2;
    x = 3;

    let y = 10;
    log(y);
    log(y + 1);
"#,
@r###"
_ini = 10
_fun = <global log>
<dead> = _fun(_ini)
_fun$1 = <global log>
_arg = 11
<dead> = _fun$1(_arg)
"###);

case!(
    downleveling_bail,
    all_passes,
    r#"
    let x = 1;
    x = 2;
    x = 3;
    if (foo) log(x);

    let y = 10;
    log(y);
    log(y + 1);
    if (bar) (function() {
        y = 5;
    })();
"#,
@r###"
_val = 3
_iff = <global foo>
<if> _iff:
    _fun$2 = <global log>
    <dead> = _fun$2(_val)
<else>:
    <empty>
_ini = 10
y <= _ini
_fun = <global log>
<dead> = _fun(_ini)
_fun$1 = <global log>
_lhs = *y
_rhs = 1
_arg = _lhs + _rhs
<dead> = _fun$1(_arg)
_iff$1 = <global bar>
<if> _iff$1:
    _val$1 = 5
    y <- _val$1
<else>:
    <empty>
"###);

case!(
    no_time_travel,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    x;
    let x = 1;
"#,
@r###"
<dead> = *x
_ini = 1
x <= _ini
"###);

case!(
    time_travel_into_function_scope,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    g = function() { return x };
    let x = 1;
"#,
@r###"
_val = <function>:
    _ret = x
    <return> _ret
<global g> <- _val
<dead> = _val
_ini = 1
x = _ini
"###);

case!(
    no_cross_case,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    switch (foo) {
        case 0:
            let x = 1;
        default:
            g = function() { return x };
    }
"#,
@r###"
_swi = <global foo>
_tst = 0
<switch> _swi:
    <case> _tst:
    _ini = 1
    x <= _ini
    <default>:
    _val = <function>:
        _ret = *x
        <return> _ret
    <global g> <- _val
    <dead> = _val
"###);

case!(
    remove_writeonly_cross_case,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    switch (foo) {
        case 0:
            let x = 1;
        default:
            g = function() { x = 2 };
    }
"#,
@r###"
_swi = <global foo>
_tst = 0
<switch> _swi:
    <case> _tst:
    <dead> = 1
    <default>:
    _val = <function>:
        _val$1 = 2
        <dead> = _val$1
    <global g> <- _val
    <dead> = _val
"###);
