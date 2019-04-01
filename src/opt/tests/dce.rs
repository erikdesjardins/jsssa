use crate::opt::dce;

case!(
    basic,
    |cx| cx.converge::<dce::Eliminate>("dce"),
    r#"
    1;
    true;
    (function() {});
    [];
    ({});
"#
);

case!(
    basic_bail,
    |cx| cx.converge::<dce::Eliminate>("dce"),
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
"#
);

case!(
    bindings,
    |cx| cx.converge::<dce::Eliminate>("dce"),
    r#"
    var x = 1;
    const y = 1;
    function z() {}
"#
);

case!(
    nested_effects,
    |cx| cx.converge::<dce::Eliminate>("dce"),
    r#"
    [{ x: call() }];
"#
);

case!(
    drop_after_jumps_1,
    |cx| cx.converge::<dce::Eliminate>("dce"),
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
"#
);

case!(
    drop_after_jumps_2,
    |cx| cx.converge::<dce::Eliminate>("dce"),
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
"#
);

case!(
    drop_after_jumps_depth,
    |cx| cx.converge::<dce::Eliminate>("dce"),
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
"#
);

case!(
    empty_blocks,
    |cx| cx.converge::<dce::Eliminate>("dce"),
    r#"
    if (x) {} else {}
    try {} catch (e) { bad(e); } finally {}
"#
);
