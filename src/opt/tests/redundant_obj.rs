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
"#
);

case!(
    basic_write_to_read,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo(something);
    something.x = x;
    g = something.x;
"#
);

case!(
    basic_write_to_write,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo(something);
    something.x = x;
    something.x = y;
"#
);

case!(
    invalid_different_props,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo();
    g1 = something.x;
    g2 = something.y;
"#
);

case!(
    invalidate_escape,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    let no_escape = { y: 2 };
    if (foo) {
        g = something;
    }
    g1 = something.x;
    g2 = no_escape.y; // forward
"#
);

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
"#
);

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
"#
);

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
"#
);

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
"#
);

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
"#
);

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
"#
);

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
"#
);

case!(
    call_receiver,
    |cx| passes!(cx),
    r#"
    let something = { x: function() {} };
    something.x(); // receives `this`: do not opt
    let something2 = { x: function() {} };
    (0, something2.x)(); // does not receive `this`: opt
"#
);
