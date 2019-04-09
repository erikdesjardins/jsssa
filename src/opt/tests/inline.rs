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
"#
);

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
"#
);

case!(
    bail_props,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        foo;
    }).x();
"#
);

case!(
    bail_new,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    new (function() {
        foo;
    })();
"#
);

case!(
    bail_this,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        if (foo) { this; }
    })();
"#
);

case!(
    bail_recursive,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function f() {
        if (foo) { f(); }
    })();
"#
);

case!(
    bail_bad_return,
    |cx| cx.converge::<inline::Inline>("inline"),
    r#"
    (function() {
        if (foo) { return; }
    })();
"#
);

case!(
    more_complex,
    all_passes,
    r#"
    g = (function f(a, b, c) {
        log();
        return a + b + c;
    })(1, 2);
"#
);

case!(
    do_not_inline_multi_use,
    all_passes,
    r#"
    const f = () => { foo; };
    f();
    f();
"#
);

case!(
    basic_inlining,
    all_passes,
    r#"
    function f(a, b, c) {
        log();
        return a + b + c + 4;
    }
    g = f(1, 2, 3);
"#
);
