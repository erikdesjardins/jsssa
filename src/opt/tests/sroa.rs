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
"#
);

case!(
    invalid_unknown_prop,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    foo();
    something.prototype;
    g = something.x;
"#
);

case!(
    invalidate_escape,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g = something;
    }
    g1 = something.x;
"#
);

case!(
    invalidate_escape_to_other_object,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    if (foo) {
        g = { foo: something };
    }
    g1 = something.x;
"#
);

case!(
    time_travel,
    |cx| passes!(cx),
    r#"
    g = function() {
        something.x;
    }
    let something = { x: 1 };
    something.x;
"#
);

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
    invalidate();
    g = obj.foo;
"#
);

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

case!(
    bail_bad_props1,
    |cx| passes!(cx),
    r#"
    let something = { __proto__: 1, x: 2 };
    g1 = something.x;
    something.x = g2;
"#
);

case!(
    bail_bad_props2,
    |cx| passes!(cx),
    r#"
    let something = { x: 2 };
    g1 = something.hasOwnProperty;
    something.x = g2;
"#
);

case!(
    bail_bad_props3,
    |cx| passes!(cx),
    r#"
    let something = { x: 2 };
    g1 = something.x;
    something.constructor = g2;
"#
);
