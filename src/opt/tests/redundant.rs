use crate::opt::redundant;

case!(
    basic_write_to_read,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    foo();
    something = x;
    let y = something;
    h = function() { return something; };
"#,
@r###"
_ini = 1
something <= _ini
_fun = <global foo>
<dead> = _fun()
_val = <global x>
something <- _val
<dead> = _val
_ini$1 = _val
y <= _ini$1
_val$1 = <function>:
    _ret = *something
    <return> _ret
<global h> <- _val$1
<dead> = _val$1
"###);

case!(
    basic_write_to_write,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    foo();
    something = x;
    something = y;
    h = function() { return something; };
"#,
@r###"
_ini = 1
something <= _ini
_fun = <global foo>
<dead> = _fun()
_val = <global x>
<dead> = _val
_val$1 = <global y>
something <- _val$1
<dead> = _val$1
_val$2 = <function>:
    _ret = *something
    <return> _ret
<global h> <- _val$2
<dead> = _val$2
"###);

case!(
    write_to_write_decl,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    something = x;
    // implicit void init
    var y = 2;
"#,
@r###"
<dead> = <void>
<dead> = 1
_val = <global x>
something <= _val
<dead> = _val
_ini = 2
y <= _ini
"###);

case!(
    forwarded_write_is_redundant,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    g = something;
    something = 2;
"#,
@r###"
_ini = 1
_val = _ini
<global g> <- _val
<dead> = _val
_val$1 = 2
something <= _val$1
<dead> = _val$1
"###);

case!(
    invalidate_inner_scope_writes,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    let bar = 2;
    let not_written = 2;
    // every ref should be forwarded except the two at the bottom
    if (something) {
        foo;
        foo = 3;
        bar;
        if (something2) {
            bar = 4;
        }
        foo;
        not_written;
    }
    foo; // do not forward
    bar; // do not forward
    not_written;
"#,
@r###"
_ini = 1
foo <= _ini
_ini$1 = 2
bar <= _ini$1
_ini$2 = 2
not_written <= _ini$2
_iff = <global something>
<if> _iff:
    <dead> = _ini
    _val = 3
    foo <- _val
    <dead> = _val
    <dead> = _ini$1
    _iff$1 = <global something2>
    <if> _iff$1:
        _val$1 = 4
        bar <- _val$1
        <dead> = _val$1
    <else>:
        <empty>
    <dead> = _val
    <dead> = _ini$2
<else>:
    <empty>
<dead> = *foo
<dead> = *bar
<dead> = _ini$2
"###);

case!(
    invalidate_inner_scope_writes_for_write,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo;
    foo = 1; // don't drop
    if (something) {
        foo = 2;
    }
    foo = 3;
"#,
@r###"
<dead> = <void>
_val = 1
foo <= _val
<dead> = _val
_iff = <global something>
<if> _iff:
    _val$2 = 2
    foo <- _val$2
    <dead> = _val$2
<else>:
    <empty>
_val$1 = 3
foo <- _val$1
<dead> = _val$1
"###);

case!(
    invalidate_inner_scope_calls,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    while (x)
        invalidate();
    g = foo;
    h = function() { return foo; };
"#,
@r###"
_ini = 1
foo <= _ini
<loop>:
    _whl = <global x>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _fun = <global invalidate>
    <dead> = _fun()
_val = *foo
<global g> <- _val
<dead> = _val
_val$1 = <function>:
    _ret = *foo
    <return> _ret
<global h> <- _val$1
<dead> = _val$1
"###);

case!(
    many_writes,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    foo = 2;
    foo = 3;
    foo = 4;
    foo = 5;
"#,
@r###"
<dead> = 1
_val = 2
<dead> = _val
_val$1 = 3
<dead> = _val$1
_val$2 = 4
<dead> = _val$2
_val$3 = 5
foo <= _val$3
<dead> = _val$3
"###);

case!(
    reads_dont_propagate_to_parent,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    invalidate();
    if (bar) {
        g = foo;
    } else {
        g = foo;
    }
    h = function() { return foo; };
"#,
@r###"
_ini = 1
foo <= _ini
_fun = <global invalidate>
<dead> = _fun()
_iff = <global bar>
<if> _iff:
    _val$1 = *foo
    <global g> <- _val$1
    <dead> = _val$1
<else>:
    _val$1 = *foo
    <global g> <- _val$1
    <dead> = _val$1
_val = <function>:
    _ret = *foo
    <return> _ret
<global h> <- _val
<dead> = _val
"###);

case!(
    switch_invalidate_local,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let outer = 1;
    switch (x) {
        case 1:
            let inner = 2;
            inner;
        case 2:
            inner; // don't forward
    }
    outer;
"#,
@r###"
_ini = 1
outer <= _ini
_swi = <global x>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _ini$1 = 2
    inner <= _ini$1
    <dead> = _ini$1
    <case> _tst$1:
    <dead> = *inner
<dead> = _ini
"###);

case!(
    across_break_write,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f;
    while (foo) {
        f += 1; // do not drop
        break;
        f = 3;
    }
"#,
@r###"
_ini = <void>
f <= _ini
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _lhs = *f
    _rhs = 1
    _val = _lhs + _rhs
    f <- _val
    <dead> = _val
    <break>
    _val$1 = 3
    f <- _val$1
    <dead> = _val$1
"###);

case!(
    across_break_write_nonlocal,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f;
    while (foo) {
        f += 1; // do not drop
        break;
        f = 3;
    }
    h = function() { return f; }
"#,
@r###"
_ini = <void>
f <= _ini
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _lhs = *f
    _rhs = 1
    _val$1 = _lhs + _rhs
    f <- _val$1
    <dead> = _val$1
    <break>
    _val$2 = 3
    f <- _val$2
    <dead> = _val$2
_val = <function>:
    _ret = *f
    <return> _ret
<global h> <- _val
<dead> = _val
"###);

case!(
    across_conditional_breaks_write,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f;
    while (foo) {
        f += 1; // do not drop
        if (bar) {
            break;
        }
        f = 3;
    }
"#,
@r###"
_ini = <void>
f <= _ini
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _lhs = *f
    _rhs = 1
    _val = _lhs + _rhs
    f <- _val
    <dead> = _val
    _iff = <global bar>
    <if> _iff:
        <break>
    <else>:
        <empty>
    _val$1 = 3
    f <- _val$1
    <dead> = _val$1
"###);

case!(
    across_deep_break_write,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f;
    outer: while (foo) {
        f += 1; // do not drop
        while (bar) while (bar) {
            break outer;
        }
        f = 3;
    }
"#,
@r###"
_ini = <void>
f <= _ini
<label outer>:
    <loop>:
        _whl = <global foo>
        <if> _whl:
            <empty>
        <else>:
            <break>
        _lhs = *f
        _rhs = 1
        _val = _lhs + _rhs
        f <- _val
        <dead> = _val
        <loop>:
            _whl$1 = <global bar>
            <if> _whl$1:
                <empty>
            <else>:
                <break>
            <loop>:
                _whl$2 = <global bar>
                <if> _whl$2:
                    <empty>
                <else>:
                    <break>
                <break outer>
        _val$1 = 3
        f <- _val$1
        <dead> = _val$1
"###);

case!(
    across_deep_break_write_nonlocal,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f;
    outer: while (foo) {
        f += 1; // do not drop
        while (bar) while (bar) {
            break outer;
        }
        f = 3;
    }
    h = function() { return f; };
"#,
@r###"
_ini = <void>
f <= _ini
<label outer>:
    <loop>:
        _whl = <global foo>
        <if> _whl:
            <empty>
        <else>:
            <break>
        _lhs = *f
        _rhs = 1
        _val$1 = _lhs + _rhs
        f <- _val$1
        <dead> = _val$1
        <loop>:
            _whl$1 = <global bar>
            <if> _whl$1:
                <empty>
            <else>:
                <break>
            <loop>:
                _whl$2 = <global bar>
                <if> _whl$2:
                    <empty>
                <else>:
                    <break>
                <break outer>
        _val$2 = 3
        f <- _val$2
        <dead> = _val$2
_val = <function>:
    _ret = *f
    <return> _ret
<global h> <- _val
<dead> = _val
"###);

case!(
    across_conditional_breaks_read,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f;
    while (foo) {
        f += 1;
        if (bar) {
            break;
        }
        f; // forward
    }
"#,
@r###"
_ini = <void>
f <= _ini
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _lhs = *f
    _rhs = 1
    _val = _lhs + _rhs
    f <- _val
    <dead> = _val
    _iff = <global bar>
    <if> _iff:
        <break>
    <else>:
        <empty>
    <dead> = _val
"###);

case!(
    into_nonlinear_scope,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let f = 1;
    for (;;) {
        f;
        f = 2;
    }
"#,
@r###"
_ini = 1
f <= _ini
<loop>:
    <dead> = *f
    _val = 2
    f <- _val
    <dead> = _val
"###);

case!(
    same_scope_not_invalidated_by_call,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    invalidate();
    if (bar) {
        g = foo;
    }
"#,
@r###"
_ini = 1
foo <= _ini
_fun = <global invalidate>
<dead> = _fun()
_iff = <global bar>
<if> _iff:
    _val = _ini
    <global g> <- _val
    <dead> = _val
<else>:
    <empty>
"###);

case!(
    revalidated_before_entering_scope,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    invalidate();
    foo = 2;
    if (bar) {
        g = foo;
    }
    h = function() { return foo; };
"#,
@r###"
_ini = 1
foo <= _ini
_fun = <global invalidate>
<dead> = _fun()
_val = 2
foo <- _val
<dead> = _val
_iff = <global bar>
<if> _iff:
    _val$2 = _val
    <global g> <- _val$2
    <dead> = _val$2
<else>:
    <empty>
_val$1 = <function>:
    _ret = *foo
    <return> _ret
<global h> <- _val$1
<dead> = _val$1
"###);

case!(
    cross_switch_read_invalidate,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    switch (bar) {
        case 1:
            foo += 1; // do not forward
        case 2:
            g = foo;
    }
"#,
@r###"
_ini = 1
foo <= _ini
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _lhs = *foo
    _rhs = 1
    _val = _lhs + _rhs
    foo <- _val
    <dead> = _val
    <case> _tst$1:
    _val$1 = *foo
    <global g> <- _val$1
    <dead> = _val$1
"###);

case!(
    cross_switch_write_invalidate,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    switch (bar) {
        case 1:
            foo += 1; // do not drop
        case 2:
            foo = 3;
    }
"#,
@r###"
_ini = 1
foo <= _ini
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _lhs = *foo
    _rhs = 1
    _val = _lhs + _rhs
    foo <- _val
    <dead> = _val
    <case> _tst$1:
    _val$1 = 3
    foo <- _val$1
    <dead> = _val$1
"###);

case!(
    cross_switch_write_invalidate_nonlocal,
    |cx| cx.run::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    switch (bar) {
        case 1:
            foo += 1; // do not drop
        case 2:
            foo = 3;
    }
    h = function() { return foo; }
"#,
@r###"
_ini = 1
foo <= _ini
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _lhs = *foo
    _rhs = 1
    _val$1 = _lhs + _rhs
    foo <- _val$1
    <dead> = _val$1
    <case> _tst$1:
    _val$2 = 3
    foo <- _val$2
    <dead> = _val$2
_val = <function>:
    _ret = *foo
    <return> _ret
<global h> <- _val
<dead> = _val
"###);
