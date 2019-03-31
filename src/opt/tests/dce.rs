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
"#
);

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
"#
);

case!(
    bindings,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    var x = 1;
    const y = 1;
    function z() {}
"#
);

case!(
    nested_effects,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    [{ x: call() }];
"#
);

case!(
    drop_after_jumps_1,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    (function() {
        if (x) {
            throw 1;
            log();
        }
        return 2;
        log();
    })();
"#
);

case!(
    drop_after_jumps_2,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    for (;;) {
        if (x) {
            continue;
            log();
        }
        if (y) {
            break;
            log();
        }
    }
"#
);

case!(
    drop_after_jumps_depth,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    (function() {
        return 2;
        (function() { log() })();
        if (x) {
            log();
        }
        log();
    })();
"#
);

case!(
    empty_blocks,
    |cx| cx.converge::<dce::Dce>("dce"),
    r#"
    if (x) {} else {}
    try {} catch (e) { log(e); } finally {}
"#
);
