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
    bail_containing_arrow_this,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        return () => this;
    })();
"#,
@r###"
_fun = <function>:
    _ret = <arrow>:
        _arr = <this>
        <return> _arr
    <return> _ret
<dead> = _fun()
"###);

case!(
    partial_bail_containing_arrow_this,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        (function() {
            return () => this;
        })();
    })();
"#,
@r###"
_mis = <void>
_fun = <function>:
    _ret = <arrow>:
        _arr = <this>
        <return> _arr
    <return> _ret
<dead> = _fun()
<dead> = _mis
"###);

case!(
    bail_containing_arrow_this_deep,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        return () => () => this;
    })();
"#,
@r###"
_fun = <function>:
    _ret = <arrow>:
        _arr = <arrow>:
            _arr$1 = <this>
            <return> _arr$1
        <return> _arr
    <return> _ret
<dead> = _fun()
"###);

case!(
    bail_containing_async_arrow_this,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        return async () => this;
    })();
"#,
@r###"
_fun = <function>:
    _ret = <arrow async>:
        _arr = <this>
        <return> _arr
    <return> _ret
<dead> = _fun()
"###);

case!(
    bail_containing_async_arrow_this_deep,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        return () => async () => this;
    })();
"#,
@r###"
_fun = <function>:
    _ret = <arrow>:
        _arr = <arrow async>:
            _arr$1 = <this>
            <return> _arr$1
        <return> _arr
    <return> _ret
<dead> = _fun()
"###);

case!(
    dont_bail_containing_innocuous_async_arrow,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        return async () => { await foo };
    })();
"#,
@r###"
<dead> = <void>
_ret = <arrow async>:
    _awa = <global foo>
    <dead> = <await> _awa
<dead> = _ret
"###);

case!(
    basic_constructor,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new function() {
        return { foo };
    }();
"#,
@r###"
<dead> = <void>
_key = "foo"
_val = <global foo>
_ret = { [_key]: _val }
<dead> = _ret
"###);

case!(
    bail_constructor_arrow,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new (() => {
        return {};
    })();
"#,
@r###"
_fun = <arrow>:
    _ret = {  }
    <return> _ret
<dead> = <new> _fun()
"###);

case!(
    bail_constructor_no_return,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new function() {
    }();
"#,
@r###"
_fun = <function>:
    <empty>
<dead> = <new> _fun()
"###);

case!(
    bail_constructor_return_bad,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new function() {
        return 1;
    }();
"#,
@r###"
_fun = <function>:
    _ret = 1
    <return> _ret
<dead> = <new> _fun()
"###);

case!(
    bail_constructor_this,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new function() {
        this;
        return {};
    }();
"#,
@r###"
_fun = <function>:
    <dead> = <this>
    _ret = {  }
    <return> _ret
<dead> = <new> _fun()
"###);

case!(
    bail_constructor_conditional_return,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new function() {
        if (foo) ;
        else return {};
    }();
"#,
@r###"
_fun = <function>:
    _iff = <global foo>
    <if> _iff:
        <empty>
    <else>:
        _ret = {  }
        <return> _ret
<dead> = <new> _fun()
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
    const f = () => { foo(); };
    f();
    f();
"#,
@r###"
_ini = <arrow>:
    _fun = <global foo>
    <dead> = _fun()
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

case!(
    no_stmts_after_return,
    all_passes,
    r#"
    function f(a, b, c) {
        log();
        return a + b + c + 4;
        console.log('drop me');
    }
    g = f(1, 2, 3);
"#,
@r###"
_fun = <global log>
<dead> = _fun()
_val = 10
<global g> <- _val
"###);
