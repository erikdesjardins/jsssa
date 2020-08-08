use crate::ir;
use crate::ir::traverse::Folder;
use crate::opt::dce;
use crate::opt::forward;
use crate::opt::mut2ssa;
use crate::opt::redundant_obj;

macro_rules! passes {
    ( $cx:ident ) => {
        $cx.run::<mut2ssa::Mut2Ssa>("mut2ssa")
            .run::<forward::Reads>("forward-reads-redundancy")
            .converge::<dce::Dce>("dce-forwarded-reads")
            .run::<redundant_obj::LoadStore>("redundant-obj")
    };
}

case!(
    basic_read_to_read,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo();
    g1 = something.x;
    g2 = something.x;
"#,
@r###"
something = {  }
_fun = <global foo>
<dead> = _fun()
_prp = "x"
_val = something[_prp]
<global g1> <- _val
<dead> = "x"
_val$1 = _val
<global g2> <- _val$1
"###);

case!(
    basic_write_to_read,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo(something);
    something.x = x;
    g = something.x;
"#,
@r###"
something = {  }
_fun = <global foo>
<dead> = _fun(something)
_prp = "x"
_val = <global x>
something[_prp] <- _val
<dead> = "x"
_val$1 = _val
<global g> <- _val$1
"###);

case!(
    basic_write_to_write,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo(something);
    something.x = x;
    something.x = y;
"#,
@r###"
something = {  }
_fun = <global foo>
<dead> = _fun(something)
<dead> = "x"
<dead> = <global x>
_prp = "x"
_val = <global y>
something[_prp] <- _val
"###);

case!(
    write_to_read_to_write,
    |cx| passes!(cx),
    r#"
    let something = {};
    something.x = 1; // drop
    g = something.x; // forward
    something.x = 2;
"#,
@r###"
something = {  }
<dead> = "x"
_val = 1
<dead> = "x"
_val$1 = _val
<global g> <- _val$1
_prp = "x"
_val$2 = 2
something[_prp] <- _val$2
"###);

case!(
    invalid_different_props,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo();
    g1 = something.x;
    g2 = something.y;
"#,
@r###"
something = {  }
_fun = <global foo>
<dead> = _fun()
_prp = "x"
_val = something[_prp]
<global g1> <- _val
_prp$1 = "y"
_val$1 = something[_prp$1]
<global g2> <- _val$1
"###);

case!(
    no_invalidate_escape,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    let no_escape = { y: 2 };
    if (foo) {
        g = something;
    }
    g1 = something.x;
    g2 = no_escape.y;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_key$1 = "y"
_val$1 = 2
<dead> = { [_key$1]: _val$1 }
_iff = <global foo>
<if> _iff:
    <global g> <- something
<else>:
    <empty>
<dead> = "x"
_val$2 = _val
<global g1> <- _val$2
<dead> = "y"
_val$3 = _val$1
<global g2> <- _val$3
"###);

case!(
    no_invalidate_escape_local,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    g = something;
    g1 = something.x;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
<global g> <- something
<dead> = "x"
_val$1 = _val
<global g1> <- _val$1
"###);

case!(
    no_invalidate_escape_local_nonlocal,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    g = something;
    g1 = something.x;
    h = function() { return something }
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
<global g> <- something
<dead> = "x"
_val$1 = _val
<global g1> <- _val$1
_val$2 = <function>:
    <return> something
<global h> <- _val$2
"###);

case!(
    no_invalidate_escape_deep,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) if (foo) {
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
    _iff$1 = <global foo>
    <if> _iff$1:
        <global g> <- something
    <else>:
        <empty>
<else>:
    <empty>
<dead> = "x"
_val$1 = _val
<global g1> <- _val$1
"###);

case!(
    no_invalidate_escape_for_write,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    something.x = 1;
    if (foo) {
        g = something;
    }
    something.x = 2;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
<dead> = "x"
<dead> = 1
_iff = <global foo>
<if> _iff:
    <global g> <- something
<else>:
    <empty>
_prp = "x"
_val$1 = 2
something[_prp] <- _val$1
"###);

case!(
    no_invalidate_escape_local_for_write,
    |cx| passes!(cx),
    r#"
    let something = {};
    something.x = 1;
    g = something;
    something.x = 2;
"#,
@r###"
something = {  }
<dead> = "x"
<dead> = 1
<global g> <- something
_prp = "x"
_val = 2
something[_prp] <- _val
"###);

case!(
    no_invalidate_escape_local_nonlocal_for_write,
    |cx| passes!(cx),
    r#"
    let something = {};
    something.x = 1;
    g = something;
    something.x = 2;
    h = function() { return something }
"#,
@r###"
something = {  }
<dead> = "x"
<dead> = 1
<global g> <- something
_prp = "x"
_val = 2
something[_prp] <- _val
_val$1 = <function>:
    <return> something
<global h> <- _val$1
"###);

case!(
    no_invalidate_escape_deep_for_write,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    something.x = 1;
    if (foo) if (foo) {
        g = something;
    }
    something.x = 2;
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
<dead> = "x"
<dead> = 1
_iff = <global foo>
<if> _iff:
    _iff$1 = <global foo>
    <if> _iff$1:
        <global g> <- something
    <else>:
        <empty>
<else>:
    <empty>
_prp = "x"
_val$1 = 2
something[_prp] <- _val$1
"###);

case!(
    invalidate_unknown_prop,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    let no_write = { y: 2 };
    if (foo) {
        something[foo] = 1;
        no_write[foo];
    }
    g1 = something.x;
    g2 = no_write.y; // forward
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_key$1 = "y"
_val$1 = 2
no_write = { [_key$1]: _val$1 }
_iff = <global foo>
<if> _iff:
    _prp$2 = <global foo>
    _val$4 = 1
    something[_prp$2] <- _val$4
    _prp$3 = <global foo>
    <dead> = no_write[_prp$3]
<else>:
    <empty>
_prp = "x"
_val$2 = something[_prp]
<global g1> <- _val$2
_prp$1 = "y"
_val$3 = no_write[_prp$1]
<global g2> <- _val$3
"###);

case!(
    invalidate_unknown_prop_nonlocal,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        something[foo] = 1;
    }
    g1 = something.x; // do not forward
    h = function() { return something }
"#,
@r###"
_key = "x"
_val = 1
something = { [_key]: _val }
_iff = <global foo>
<if> _iff:
    _prp$1 = <global foo>
    _val$3 = 1
    something[_prp$1] <- _val$3
<else>:
    <empty>
_prp = "x"
_val$1 = something[_prp]
<global g1> <- _val$1
_val$2 = <function>:
    <return> something
<global h> <- _val$2
"###);

case!(
    invalidate_inner_scope_writes,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1, bar: 2, not_written: 2 };
    // every ref should be forwarded except the two at the bottom
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
    obj.foo; // do not forward
    obj.bar; // do not forward
    obj.not_written;
"#,
@r###"
_key = "foo"
_val = 1
_key$1 = "bar"
_val$1 = 2
_key$2 = "not_written"
_val$2 = 2
obj = { [_key]: _val, [_key$1]: _val$1, [_key$2]: _val$2 }
_iff = <global something>
<if> _iff:
    <dead> = "foo"
    <dead> = _val
    _prp$2 = "foo"
    _val$3 = 3
    obj[_prp$2] <- _val$3
    <dead> = "bar"
    <dead> = _val$1
    _iff$1 = <global something2>
    <if> _iff$1:
        _prp$3 = "bar"
        _val$4 = 4
        obj[_prp$3] <- _val$4
    <else>:
        <empty>
    <dead> = "foo"
    <dead> = _val$3
    <dead> = "not_written"
    <dead> = _val$2
<else>:
    <empty>
_prp = "foo"
<dead> = obj[_prp]
_prp$1 = "bar"
<dead> = obj[_prp$1]
<dead> = "not_written"
<dead> = _val$2
"###);

case!(
    invalidate_inner_scope_writes_nonlocal,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    if (something) {
        obj.foo = 2;
    }
    obj.foo; // do not forward
    h = function() { return obj }
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
_iff = <global something>
<if> _iff:
    _prp$1 = "foo"
    _val$2 = 2
    obj[_prp$1] <- _val$2
<else>:
    <empty>
_prp = "foo"
<dead> = obj[_prp]
_val$1 = <function>:
    <return> obj
<global h> <- _val$1
"###);

case!(
    invalidate_inner_scope_writes_for_write,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    obj.foo = 1; // do not remove
    if (something) {
        obj.foo = g;
    }
    obj.foo = 2;
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
_prp = "foo"
_val$1 = 1
obj[_prp] <- _val$1
_iff = <global something>
<if> _iff:
    _prp$2 = "foo"
    _val$3 = <global g>
    obj[_prp$2] <- _val$3
<else>:
    <empty>
_prp$1 = "foo"
_val$2 = 2
obj[_prp$1] <- _val$2
"###);

case!(
    invalidate_inner_scope_writes_for_write_deep,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    obj.foo = 1; // do not remove
    if (something) if (something) {
        obj.foo = g;
    }
    obj.foo = 2;
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
_prp = "foo"
_val$1 = 1
obj[_prp] <- _val$1
_iff = <global something>
<if> _iff:
    _iff$1 = <global something>
    <if> _iff$1:
        _prp$2 = "foo"
        _val$3 = <global g>
        obj[_prp$2] <- _val$3
    <else>:
        <empty>
<else>:
    <empty>
_prp$1 = "foo"
_val$2 = 2
obj[_prp$1] <- _val$2
"###);

case!(
    invalidate_calls,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    invalidate();
    obj.foo; // do not forward
    h = function() { return obj; };
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
_fun = <global invalidate>
<dead> = _fun()
_prp = "foo"
<dead> = obj[_prp]
_val$1 = <function>:
    <return> obj
<global h> <- _val$1
"###);

case!(
    invalidate_calls_for_write,
    |cx| passes!(cx),
    r#"
    let obj = {};
    obj.foo = 1; // do not drop
    invalidate();
    obj.foo = 2;
    h = function() { return obj; };
"#,
@r###"
obj = {  }
_prp = "foo"
_val = 1
obj[_prp] <- _val
_fun = <global invalidate>
<dead> = _fun()
_prp$1 = "foo"
_val$1 = 2
obj[_prp$1] <- _val$1
_val$2 = <function>:
    <return> obj
<global h> <- _val$2
"###);

case!(
    invalidate_inner_scope_calls,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    while (x)
        invalidate();
    obj.foo; // do not forward
    h = function() { return obj; };
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
<loop>:
    _whl = <global x>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _fun = <global invalidate>
    <dead> = _fun()
_prp = "foo"
<dead> = obj[_prp]
_val$1 = <function>:
    <return> obj
<global h> <- _val$1
"###);

case!(
    invalidate_deep_inner_scope_calls,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    while (x) while (x)
        invalidate();
    obj.foo; // do not forward
    h = function() { return obj; };
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
<loop>:
    _whl = <global x>
    <if> _whl:
        <empty>
    <else>:
        <break>
    <loop>:
        _whl$1 = <global x>
        <if> _whl$1:
            <empty>
        <else>:
            <break>
        _fun = <global invalidate>
        <dead> = _fun()
_prp = "foo"
<dead> = obj[_prp]
_val$1 = <function>:
    <return> obj
<global h> <- _val$1
"###);

case!(
    invalidate_deep_inner_scope_calls_for_write,
    |cx| passes!(cx),
    r#"
    let obj = {};
    obj.foo = 1; // do not drop
    if (x) if (x)
        invalidate();
    obj.foo = 2;
    h = function() { return obj; };
"#,
@r###"
obj = {  }
_prp = "foo"
_val = 1
obj[_prp] <- _val
_iff = <global x>
<if> _iff:
    _iff$1 = <global x>
    <if> _iff$1:
        _fun = <global invalidate>
        <dead> = _fun()
    <else>:
        <empty>
<else>:
    <empty>
_prp$1 = "foo"
_val$1 = 2
obj[_prp$1] <- _val$1
_val$2 = <function>:
    <return> obj
<global h> <- _val$2
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
    g = obj;
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
<dead> = "foo"
<dead> = 2
<dead> = "foo"
<dead> = 3
<dead> = "foo"
<dead> = 4
_prp = "foo"
_val$1 = 5
obj[_prp] <- _val$1
<global g> <- obj
"###);

case!(
    reads_dont_propagate_to_parent,
    |cx| passes!(cx),
    r#"
    let obj = { foo: 1 };
    invalidate();
    if (bar) {
        g = obj.foo;
    } else {
        g = obj.foo;
    }
    h = function() { return obj; }
"#,
@r###"
_key = "foo"
_val = 1
obj = { [_key]: _val }
_fun = <global invalidate>
<dead> = _fun()
_iff = <global bar>
<if> _iff:
    _prp = "foo"
    _val$2 = obj[_prp]
    <global g> <- _val$2
<else>:
    _prp = "foo"
    _val$2 = obj[_prp]
    <global g> <- _val$2
_val$1 = <function>:
    <return> obj
<global h> <- _val$1
"###);

case!(
    switch_invalidate_local,
    |cx| passes!(cx),
    r#"
    let a = { outer: 1 };
    switch (x) {
        case 1:
            a.inner = 2;
            a.inner;
        case 2:
            a.inner; // don't forward
    }
    a.outer;
"#,
@r###"
_key = "outer"
_val = 1
a = { [_key]: _val }
_swi = <global x>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _prp = "inner"
    _val$1 = 2
    a[_prp] <- _val$1
    <dead> = "inner"
    <dead> = _val$1
    <case> _tst$1:
    _prp$1 = "inner"
    <dead> = a[_prp$1]
<dead> = "outer"
<dead> = _val
"###);

#[derive(Debug, Default)]
struct DropDeadSsaReads;

impl Folder for DropDeadSsaReads {
    type Output = Option<ir::Stmt>;

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                ref target,
                expr: ir::Expr::Read { source: _ },
            } if target.used().is_never() => None,
            _ => Some(stmt),
        }
    }
}

case!(
    across_break_write,
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads-redundancy")
        .run::<DropDeadSsaReads>("avoid-premature-invalidation")
        .run::<redundant_obj::LoadStore>("redundant-obj"),
    r#"
    let o = { f: 0 };
    while (foo) {
        o.f += 1; // do not drop
        break;
        o.f = 3;
    }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$1 = _lhs + _rhs
    o[_prp] <- _val$1
    <break>
    _prp$1 = "f"
    _val$2 = 3
    o[_prp$1] <- _val$2
"###);

case!(
    across_conditional_breaks_write,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    while (foo) {
        o.f += 1; // do not drop
        if (bar) {
            break;
        }
        o.f = 3;
    }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$1 = _lhs + _rhs
    o[_prp] <- _val$1
    _iff = <global bar>
    <if> _iff:
        <break>
    <else>:
        <empty>
    _prp$1 = "f"
    _val$2 = 3
    o[_prp$1] <- _val$2
"###);

case!(
    across_break_write_nonlocal,
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads-redundancy")
        .run::<DropDeadSsaReads>("avoid-premature-invalidation")
        .run::<redundant_obj::LoadStore>("redundant-obj"),
    r#"
    let o = { f: 0 };
    while (foo) {
        o.f += 1; // do not drop
        break;
        o.f = 3;
    }
    h = function() { return o; }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$2 = _lhs + _rhs
    o[_prp] <- _val$2
    <break>
    _prp$1 = "f"
    _val$3 = 3
    o[_prp$1] <- _val$3
_val$1 = <function>:
    <return> o
<global h> <- _val$1
"###);

case!(
    across_deep_break_write,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    outer: while (foo) {
        o.f += 1; // do not drop
        while (bar) while (bar) {
            break outer;
        }
        o.f = 3;
    }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
<label outer>:
    <loop>:
        _whl = <global foo>
        <if> _whl:
            <empty>
        <else>:
            <break>
        _prp = "f"
        _lhs = o[_prp]
        _rhs = 1
        _val$1 = _lhs + _rhs
        o[_prp] <- _val$1
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
        _prp$1 = "f"
        _val$2 = 3
        o[_prp$1] <- _val$2
"###);

case!(
    across_deep_break_write_nonlocal,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    outer: while (foo) {
        o.f += 1; // do not drop
        while (bar) while (bar) {
            break outer;
        }
        o.f = 3;
    }
    h = function() { return o; }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
<label outer>:
    <loop>:
        _whl = <global foo>
        <if> _whl:
            <empty>
        <else>:
            <break>
        _prp = "f"
        _lhs = o[_prp]
        _rhs = 1
        _val$2 = _lhs + _rhs
        o[_prp] <- _val$2
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
        _prp$1 = "f"
        _val$3 = 3
        o[_prp$1] <- _val$3
_val$1 = <function>:
    <return> o
<global h> <- _val$1
"###);

case!(
    across_conditional_breaks_read,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    while (foo) {
        o.f += 1;
        if (bar) {
            break;
        }
        g = o.f; // forward
    }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
<loop>:
    _whl = <global foo>
    <if> _whl:
        <empty>
    <else>:
        <break>
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$1 = _lhs + _rhs
    o[_prp] <- _val$1
    _iff = <global bar>
    <if> _iff:
        <break>
    <else>:
        <empty>
    <dead> = "f"
    _val$2 = _val$1
    <global g> <- _val$2
"###);

case!(
    into_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let obj = { f: 1 };
    for (;;) {
        obj.f;
        obj.f = 2;
    }
"#,
@r###"
_key = "f"
_val = 1
obj = { [_key]: _val }
<loop>:
    _prp = "f"
    <dead> = obj[_prp]
    _prp$1 = "f"
    _val$1 = 2
    obj[_prp$1] <- _val$1
"###);

case!(
    same_scope_not_invalidated_by_call,
    |cx| passes!(cx),
    r#"
    let obj = { f: 1 };
    invalidate();
    if (bar) {
        g = obj.f;
    }
    i = obj.f;
"#,
@r###"
_key = "f"
_val = 1
obj = { [_key]: _val }
_fun = <global invalidate>
<dead> = _fun()
_iff = <global bar>
<if> _iff:
    _prp$1 = "f"
    _val$2 = obj[_prp$1]
    <global g> <- _val$2
<else>:
    <empty>
_prp = "f"
_val$1 = obj[_prp]
<global i> <- _val$1
"###);

case!(
    revalidated_before_entering_scope,
    |cx| passes!(cx),
    r#"
    let obj = { f: 1 };
    invalidate();
    obj.f = 2;
    if (bar) {
        g = obj.f;
    }
    i = obj.f;
    h = function() { return obj; };
"#,
@r###"
_key = "f"
_val = 1
obj = { [_key]: _val }
_fun = <global invalidate>
<dead> = _fun()
_prp = "f"
_val$1 = 2
obj[_prp] <- _val$1
_iff = <global bar>
<if> _iff:
    <dead> = "f"
    _val$4 = _val$1
    <global g> <- _val$4
<else>:
    <empty>
<dead> = "f"
_val$2 = _val$1
<global i> <- _val$2
_val$3 = <function>:
    <return> obj
<global h> <- _val$3
"###);

case!(
    cross_switch_read_invalidate,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    switch (bar) {
        case 1:
            o.f += 1; // do not forward
        case 2:
            g = o.f;
    }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$1 = _lhs + _rhs
    o[_prp] <- _val$1
    <case> _tst$1:
    _prp$1 = "f"
    _val$2 = o[_prp$1]
    <global g> <- _val$2
"###);

case!(
    cross_switch_read_invalidate_nonlocal,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    switch (bar) {
        case 1:
            o.f += 1; // do not forward
        case 2:
            g = o.f;
    }
    h = function() { return o; }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$2 = _lhs + _rhs
    o[_prp] <- _val$2
    <case> _tst$1:
    _prp$1 = "f"
    _val$3 = o[_prp$1]
    <global g> <- _val$3
_val$1 = <function>:
    <return> o
<global h> <- _val$1
"###);

case!(
    cross_switch_write_invalidate,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    switch (bar) {
        case 1:
            o.f += 1; // do not drop
        case 2:
            o.f = 3;
    }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$1 = _lhs + _rhs
    o[_prp] <- _val$1
    <case> _tst$1:
    _prp$1 = "f"
    _val$2 = 3
    o[_prp$1] <- _val$2
"###);

case!(
    cross_switch_write_invalidate_nonlocal,
    |cx| passes!(cx),
    r#"
    let o = { f: 0 };
    switch (bar) {
        case 1:
            o.f += 1; // do not drop
        case 2:
            o.f = 3;
    }
    h = function() { return o; }
"#,
@r###"
_key = "f"
_val = 0
o = { [_key]: _val }
_swi = <global bar>
_tst = 1
_tst$1 = 2
<switch> _swi:
    <case> _tst:
    _prp = "f"
    _lhs = o[_prp]
    _rhs = 1
    _val$2 = _lhs + _rhs
    o[_prp] <- _val$2
    <case> _tst$1:
    _prp$1 = "f"
    _val$3 = 3
    o[_prp$1] <- _val$3
_val$1 = <function>:
    <return> o
<global h> <- _val$1
"###);

case!(
    call_receiver,
    |cx| passes!(cx),
    r#"
    let something = { x: function() {} };
    something.x(); // receives `this`: do not opt
    let something2 = { x: function() {} };
    (0, something2.x)(); // does not receive `this`: opt
"#,
@r###"
_key = "x"
_val = <function>:
    <empty>
_obj = { [_key]: _val }
_prp = "x"
<dead> = _obj[_prp]()
_key$1 = "x"
_val$1 = <function>:
    <empty>
<dead> = { [_key$1]: _val$1 }
<dead> = "x"
_fun = _val$1
<dead> = _fun()
"###);

case!(
    aliased_by_this,
    all_passes,
    r#"
    var o = {
        x: function() {
            o.x = 1;
            this.x = 2;
            return o.x;
        }
    };
    g = o;
"#,
@r###"
_key = "x"
_val = <function>:
    _prp = "x"
    _val$1 = 1
    _ini[_prp] <- _val$1
    _obj = <this>
    _prp$1 = "x"
    _val$2 = 2
    _obj[_prp$1] <- _val$2
    _prp$2 = "x"
    _ret = _ini[_prp$2]
    <return> _ret
_ini = { [_key]: _val }
<global g> <- _ini
"###);

case!(
    aliasing,
    all_passes,
    r#"
    var o = {};
    g = o;
    o.x = 1;
    g.x = 2;
    h = o.x; // do not forward
"#,
@r###"
_ini = {  }
<global g> <- _ini
_prp = "x"
_val = 1
_ini[_prp] <- _val
_obj = <global g>
_prp$1 = "x"
_val$1 = 2
_obj[_prp$1] <- _val$1
_prp$2 = "x"
_val$2 = _ini[_prp$2]
<global h> <- _val$2
"###);

case!(
    fully_unknown,
    all_passes,
    r#"
    g = function(a, b, c) {
      a.foo = 1;
      return a.foo + 1;
    }
"#,
@r###"
_val = <function>:
    a = <argument 0>
    _prp = "foo"
    _val$1 = 1
    a[_prp] <- _val$1
    _ret = 2
    <return> _ret
<global g> <- _val
"###);
