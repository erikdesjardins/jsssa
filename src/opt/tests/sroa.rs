use crate::opt::dce;
use crate::opt::forward;
use crate::opt::mut2ssa;
use crate::opt::sroa;

macro_rules! passes {
    ( $cx:ident ) => {
        $cx.run::<mut2ssa::Mut2Ssa>("mut2ssa")
            .run::<forward::Reads>("forward-reads-redundancy")
            .converge::<dce::Dce>("dce-forwarded-reads")
            .run::<sroa::Replace>("scalar-replace")
    };
}

case!(
    basic,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    foo();
    g1 = something.x;
    something.x = g2;
"#,
@r###"
<dead> = "x"
_val = 1
<dead> = <void>
x <= _val
_fun = <global foo>
<dead> = _fun()
<dead> = "x"
_val$1 = *x
<global g1> <- _val$1
<dead> = "x"
_val$2 = <global g2>
x <- _val$2
"###);

case!(
    unknown_prop,
    |cx| passes!(cx),
    r#"
    let something = {};
    g = something.foo;
"#,
@r###"
_mis = <void>
foo <= _mis
<dead> = "foo"
_val = *foo
<global g> <- _val
"###);

case!(
    unknown_prop_before_decl,
    |cx| passes!(cx),
    r#"
    g = function() {
        something.foo = 2;
    }
    let something = {};
"#,
@r###"
_val = <function>:
    <dead> = "foo"
    _val$1 = 2
    foo <- _val$1
<global g> <- _val
_mis = <void>
foo <= _mis
"###);

case!(
    invalidate_escape,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g = something;
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    <global g> <- something
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    invalidate_escape_after_safe_use,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    g1 = () => something.x;
    if (foo) {
        g = something;
    }
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_val$1 = <arrow>:
    _prp = "x"
    _arr = something[_prp]
    <return> _arr
<global g1> <- _val$1
_iff = <global foo>
<if> _iff:
    <global g> <- something
<else>:
    <empty>
"###);

case!(
    invalidate_escape_to_other_object,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g = { foo: something };
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _key$1 = "foo"
    _val$2 = { [_key$1]: something }
    <global g> <- _val$2
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    invalidate_escape_to_other_object_write,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g.foo = something;
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _obj = <global g>
    _prp$1 = "foo"
    _obj[_prp$1] <- something
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    invalidate_escape_through_prop_read,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        h = g[something];
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _obj = <global g>
    _val$2 = _obj[something]
    <global h> <- _val$2
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    invalidate_escape_through_prop_write,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g[something] = foo;
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _obj = <global g>
    _val$2 = <global foo>
    _obj[something] <- _val$2
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    invalidate_escape_through_prop_call,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g[something]();
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _obj = <global g>
    <dead> = _obj[something]()
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    invalidate_escape_through_prop_call_arg,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g.x(something);
    }
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _obj = <global g>
    _prp$1 = "x"
    <dead> = _obj[_prp$1](something)
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
"###);

case!(
    time_travel,
    |cx| passes!(cx),
    r#"
    g = function() {
        something.x;
    }
    let something = { x: 1 };
    something.x;
"#,
@r###"
_val = <function>:
    <dead> = "x"
    <dead> = *x
<global g> <- _val
<dead> = "x"
_val$1 = 1
<dead> = <void>
x <= _val$1
<dead> = "x"
<dead> = *x
"###);

case!(
    complex,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1, bar: 2, not_written: 2 };
    if (something) {
        obj.foo;
        obj.foo = 3;
        obj.bar;
        if (something2) {
            obj.bar = 4;
        }
        obj.foo;
        obj.not_written;
    }
    obj.foo;
    obj.bar;
    obj.not_written;
"#,
@r###"
<dead> = "foo"
_val = 1
<dead> = "bar"
_val$1 = 2
<dead> = "not_written"
_val$2 = 2
<dead> = <void>
foo <= _val
bar <= _val$1
not_written <= _val$2
_iff = <global something>
<if> _iff:
    <dead> = "foo"
    <dead> = *foo
    <dead> = "foo"
    _val$3 = 3
    foo <- _val$3
    <dead> = "bar"
    <dead> = *bar
    _iff$1 = <global something2>
    <if> _iff$1:
        <dead> = "bar"
        _val$4 = 4
        bar <- _val$4
    <else>:
        <empty>
    <dead> = "foo"
    <dead> = *foo
    <dead> = "not_written"
    <dead> = *not_written
<else>:
    <empty>
<dead> = "foo"
<dead> = *foo
<dead> = "bar"
<dead> = *bar
<dead> = "not_written"
<dead> = *not_written
"###);

case!(
    many_writes,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    obj.foo = 2;
    obj.foo = 3;
    obj.foo = 4;
    obj.foo = 5;
    invalidate();
    g = obj.foo;
"#,
@r###"
<dead> = "foo"
_val = 1
<dead> = <void>
foo <= _val
<dead> = "foo"
_val$1 = 2
foo <- _val$1
<dead> = "foo"
_val$2 = 3
foo <- _val$2
<dead> = "foo"
_val$3 = 4
foo <- _val$3
<dead> = "foo"
_val$4 = 5
foo <- _val$4
_fun = <global invalidate>
<dead> = _fun()
<dead> = "foo"
_val$5 = *foo
<global g> <- _val$5
"###);

case!(
    switch_local,
    |cx| passes!(cx),
    r#"
    let a = { inner: 0, outer: 1 };
    switch (x) {
        case 1:
            a.inner = 2;
            a.inner;
        case 2:
            a.inner;
    }
    a.outer;
"#,
@r###"
<dead> = "inner"
_val = 0
<dead> = "outer"
_val$1 = 1
<dead> = <void>
inner <= _val
outer <= _val$1
_swi = <global x>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    <dead> = "inner"
    _val$2 = 2
    inner <- _val$2
    <dead> = "inner"
    <dead> = *inner
    <case> _tst$1:
    <dead> = "inner"
    <dead> = *inner
<dead> = "outer"
<dead> = *outer
"###);

case!(
    bail_call_receiver,
    |cx| passes!(cx),
    r#"
    let something = { x: function() { g = this; } };
    something.x(); // receives `this`: do not opt
    let something2 = { x: function() {} };
    (0, something2.x)(); // does not receive `this`: opt
"#,
@r###"
_key = "x"
_val = <function>:
    _val$2 = <this>
    <global g> <- _val$2
_obj = { [_key]: _val }
_prp = "x"
<dead> = _obj[_prp]()
<dead> = "x"
_val$1 = <function>:
    <empty>
<dead> = <void>
x <= _val$1
<dead> = "x"
_fun = *x
<dead> = _fun()
"###);

case!(
    bail_call_receiver_reassign,
    |cx| passes!(cx),
    r#"
    let something = { x: function() {} };
    something.x = function() { g = this; };
    something.x(); // receives `this`: do not opt
"#,
@r###"
_key = "x"
_val = <function>:
    <empty>
something = { [_key]: _val }
_prp = "x"
_val$1 = <function>:
    _val$2 = <this>
    <global g> <- _val$2
something[_prp] <- _val$1
_prp$1 = "x"
<dead> = something[_prp$1]()
"###);

case!(
    bail_call_receiver_reassign_time_travel_call,
    |cx| passes!(cx),
    r#"
    g = () => something.x(); // receives `this`: do not opt
    let something = { x: function() {} };
    something.x = function() { g = this; };
"#,
@r###"
_val = <arrow>:
    _prp$1 = "x"
    _arr = something[_prp$1]()
    <return> _arr
<global g> <- _val
_key = "x"
_val$1 = <function>:
    <empty>
something = { [_key]: _val$1 }
_prp = "x"
_val$2 = <function>:
    _val$3 = <this>
    <global g> <- _val$3
something[_prp] <- _val$2
"###);

case!(
    bail_call_receiver_reassign_time_travel_access,
    |cx| passes!(cx),
    r#"
    g = () => something.x;
    let something = { x: function() {} };
    something.x = function() { g = this; };
    something.x(); // receives `this`: do not opt
"#,
@r###"
_val = <arrow>:
    _prp$2 = "x"
    _arr = something[_prp$2]
    <return> _arr
<global g> <- _val
_key = "x"
_val$1 = <function>:
    <empty>
something = { [_key]: _val$1 }
_prp = "x"
_val$2 = <function>:
    _val$3 = <this>
    <global g> <- _val$3
something[_prp] <- _val$2
_prp$1 = "x"
<dead> = something[_prp$1]()
"###);

case!(
    bail_call_receiver_reassign_unknown,
    |cx| passes!(cx),
    r#"
    let something = { x: function() {} };
    something.x = g;
    something.x(); // receives `this`: do not opt
"#,
@r###"
_key = "x"
_val = <function>:
    <empty>
something = { [_key]: _val }
_prp = "x"
_val$1 = <global g>
something[_prp] <- _val$1
_prp$1 = "x"
<dead> = something[_prp$1]()
"###);

case!(
    safe_call_receiver,
    |cx| passes!(cx),
    r#"
    let something = { x: function() {} };
    something.x(); // receives `this` but doesn't use it: opt
"#,
@r###"
<dead> = "x"
_val = <function>:
    <empty>
<dead> = <void>
x <= _val
<dead> = "x"
x$1 = *x
<dead> = x$1()
"###);

case!(
    safe_call_receiver_reassign,
    |cx| passes!(cx),
    r#"
    let something = { x: function() {} };
    something.x = function() {};
    something.x(); // receives `this` but doesn't use it: opt
"#,
@r###"
<dead> = "x"
_val = <function>:
    <empty>
<dead> = <void>
x <= _val
<dead> = "x"
_val$1 = <function>:
    <empty>
x <- _val$1
<dead> = "x"
x$1 = *x
<dead> = x$1()
"###);

case!(
    bail_bad_props1,
    |cx| passes!(cx),
    r#"
    let something = { __proto__: 1, x: 2 };
    g1 = something.x;
    something.x = g2;
"#,
@r###"
_key = "__proto__"
_val = 1
_key$1 = "x"
_val$1 = 2
something = { [_key]: _val, [_key$1]: _val$1 }
_prp = "x"
_val$2 = something[_prp]
<global g1> <- _val$2
_prp$1 = "x"
_val$3 = <global g2>
something[_prp$1] <- _val$3
"###);

case!(
    bail_bad_props2,
    |cx| passes!(cx),
    r#"
    let something = { x: 2 };
    g1 = something.hasOwnProperty;
    something.x = g2;
"#,
@r###"
_key = "x"
_val = 2
something = { [_key]: _val }
_prp = "hasOwnProperty"
_val$1 = something[_prp]
<global g1> <- _val$1
_prp$1 = "x"
_val$2 = <global g2>
something[_prp$1] <- _val$2
"###);

case!(
    bail_bad_props3,
    |cx| passes!(cx),
    r#"
    let something = { x: 2 };
    g1 = something.x;
    something.constructor = g2;
"#,
@r###"
_key = "x"
_val = 2
something = { [_key]: _val }
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
_prp$1 = "constructor"
_val$2 = <global g2>
something[_prp$1] <- _val$2
"###);
