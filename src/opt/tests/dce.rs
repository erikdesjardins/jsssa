use crate::opt::dce;

case!(
    basic,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    1;
    true;
    (function() {});
    [];
    ({});
"#,
@"<empty>");

case!(
    basic_bail,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    x;
    const foo;
    foo.bar;
    delete foo.bar;
    foo();
    (function* baz() {
        yield;
    })();
    (async function baz2() {
        await 1;
    })();
"#,
@r###"
<dead> = <global x>
_ini = <void>
foo <= _ini
_obj = *foo
_prp = "bar"
<dead> = _obj[_prp]
_obj$1 = *foo
_prp$1 = "bar"
<dead> = <delete> _obj$1[_prp$1]
_fun = *foo
<dead> = _fun()
_fun$1 = <function generator>:
    _yld = <void>
    <dead> = <yield> _yld
<dead> = _fun$1()
_fun$2 = <function async>:
    _awa = 1
    <dead> = <await> _awa
<dead> = _fun$2()
"###);

case!(
    bindings,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    var x = 1;
    const y = 1;
    function z() {}
"#,
@r###"
_ini = <void>
z <= _ini
_ini$1 = <void>
x <= _ini$1
_fun = <function>:
    <empty>
z <- _fun
_ini$2 = 1
x <- _ini$2
"###);

case!(
    nested_effects,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    [{ x: call() }];
"#,
@r###"
_fun = <global call>
<dead> = _fun()
"###);

case!(
    drop_after_jumps_1,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    (function() {
        good;
        if (x) {
            good;
            throw 1;
            bad;
        }
        good;
        return 2;
        bad;
    })();
"#,
@r###"
_fun = <function>:
    <dead> = <global good>
    _iff = <global x>
    <if> _iff:
        <dead> = <global good>
        _thr = 1
        <throw> _thr
    <else>:
        <empty>
    <dead> = <global good>
    _ret = 2
    <return> _ret
<dead> = _fun()
"###);

case!(
    drop_after_jumps_2,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    for (;;) {
        good;
        if (x) {
            good;
            continue;
            bad;
        }
        good;
        if (y) {
            good;
            break;
            bad;
        }
        good;
    }
"#,
@r###"
<loop>:
    <dead> = <global good>
    _iff = <global x>
    <if> _iff:
        <dead> = <global good>
        <continue>
    <else>:
        <empty>
    <dead> = <global good>
    _iff$1 = <global y>
    <if> _iff$1:
        <dead> = <global good>
        <break>
    <else>:
        <empty>
    <dead> = <global good>
"###);

case!(
    drop_after_jumps_depth,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    (function() {
        good;
        return 2;
        (function() { bad; })();
        if (x) {
            bad;
        }
        bad;
    })();
"#,
@r###"
_fun = <function>:
    <dead> = <global good>
    _ret = 2
    <return> _ret
<dead> = _fun()
"###);

case!(
    dont_drop_hoisted_fns,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    (function() {
        foo();
        return;
        function foo() {
            log();
        }
    })();
"#,
@r###"
_fun = <function>:
    _ini = <void>
    foo <= _ini
    _fun$1 = <function>:
        _fun$3 = <global log>
        <dead> = _fun$3()
    foo <- _fun$1
    _fun$2 = *foo
    <dead> = _fun$2()
    _ret = <void>
    <return> _ret
<dead> = _fun()
"###);

case!(
    empty_blocks,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    if (x) {} else {}
    try {} catch (e) { bad(e); } finally {}
"#,
@"<dead> = <global x>");

case!(
    dont_drop_after_switch_break,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    switch (x) {
        case 1:
            break;
            drop_me();
        case 2:
            log();
    }
"#,
@r###"
_swi = <global x>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    <break>
    <case> _tst$1:
    _fun = <global log>
    <dead> = _fun()
"###);

case!(
    do_not_eliminate_for_in_with_assignments,
    all_passes,
    r#"
    let x = {};
    x.y = 1;
    for (let k in x) log(k);
"#,
@r###"
_ini = {  }
_prp = "y"
_val = 1
_ini[_prp] <- _val
<foreach in> _ini:
    _for = <argument 0>
    _fun = <global log>
    <dead> = _fun(_for)
"###);
