use crate::opt::inline;

case!(
    basic,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        foo;
    })();
    (() => {
        foo;
    })();
"#,
@r###"
_mis = <void>
<dead> = <global foo>
<dead> = _mis
_mis$1 = <void>
<dead> = <global foo>
<dead> = _mis$1
"###);

case!(
    bail_async_gen,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function*() {
        foo;
    })();
    (async function() {
        foo;
    })();
    (async () => {
        foo;
    })();
"#,
@r###"
_fun = <function generator>:
    <dead> = <global foo>
<dead> = _fun()
_fun$1 = <function async>:
    <dead> = <global foo>
<dead> = _fun$1()
_fun$2 = <arrow async>:
    <dead> = <global foo>
<dead> = _fun$2()
"###);

case!(
    bail_props,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        foo;
    }).x();
"#,
@r###"
_obj = <function>:
    <dead> = <global foo>
_prp = "x"
<dead> = _obj[_prp]()
"###);

case!(
    bail_new,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new (function() {
        foo;
    })();
"#,
@r###"
_fun = <function>:
    <dead> = <global foo>
<dead> = <new> _fun()
"###);

case!(
    bail_this,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        if (foo) { this; }
    })();
"#,
@r###"
_fun = <function>:
    _iff = <global foo>
    <if> _iff:
        <dead> = <this>
    <else>:
        <empty>
<dead> = _fun()
"###);

case!(
    bail_recursive,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function f() {
        if (foo) { f(); }
    })();
"#,
@r###"
_fun = <function>:
    f = <current function>
    f$1 <= f
    _iff = <global foo>
    <if> _iff:
        _fun$1 = *f$1
        <dead> = _fun$1()
    <else>:
        <empty>
<dead> = _fun()
"###);

case!(
    bail_bad_return,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        if (foo) { return; }
    })();
"#,
@r###"
_fun = <function>:
    _iff = <global foo>
    <if> _iff:
        _ret = <void>
        <return> _ret
    <else>:
        <empty>
<dead> = _fun()
"###);

case!(
    more_complex,
    all_passes,
    r#"
    g = (function f(a, b, c) {
        log();
        return a + b + c;
    })(1, 2);
"#,
@r###"
_mis = <void>
_fun = <global log>
<dead> = _fun()
_lhs = 3
_val = _lhs + _mis
<global g> <- _val
"###);

case!(
    do_not_inline_multi_use,
    all_passes,
    r#"
    const f = () => { foo; };
    f();
    f();
"#,
@r###"
_ini = <arrow>:
    <dead> = <global foo>
<dead> = _ini()
<dead> = _ini()
"###);

case!(
    basic_inlining,
    all_passes,
    r#"
    function f(a, b, c) {
        log();
        return a + b + c + 4;
    }
    g = f(1, 2, 3);
"#,
@r###"
_fun = <global log>
<dead> = _fun()
_val = 10
<global g> <- _val
"###);
