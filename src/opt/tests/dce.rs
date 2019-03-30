use crate::opt::dce;

case!(
    basic,
    |cx| cx.run_to_convergence(|cx| cx.run(dce::Dce)),
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
    |cx| cx.run_to_convergence(|cx| cx.run(dce::Dce)),
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
    |cx| cx.run_to_convergence(|cx| cx.run(dce::Dce)),
    r#"
    var x = 1;
    const y = 1;
    function z() {}
"#
);

case!(
    nested_effects,
    |cx| cx.run_to_convergence(|cx| cx.run(dce::Dce)),
    r#"
    [{ x: call() }];
"#
);
