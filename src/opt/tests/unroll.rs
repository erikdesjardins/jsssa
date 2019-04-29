use crate::opt::dce;
use crate::opt::forward;
use crate::opt::mut2ssa;
use crate::opt::unroll;

macro_rules! passes {
    ( $cx:ident ) => {
        $cx.run::<mut2ssa::Mut2Ssa>("mut2ssa")
            .run::<forward::Reads>("forward-reads-redundancy")
            .converge::<dce::Dce>("dce-forwarded-reads")
            .run::<unroll::Loops>("unroll-loops")
    };
}

case!(
    basic_zero,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    basic_one,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    other_ops_ok,
    |cx| passes!(cx),
    r#"
    let something = { x: 1 };
    g = 1;
    y.x = 2;
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    bail_mutate,
    |cx| passes!(cx),
    r#"
    let something = {};
    something.x = 1;
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    bail_escape,
    |cx| passes!(cx),
    r#"
    let something = {};
    g = something;
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    bail_call,
    |cx| passes!(cx),
    r#"
    let something = {};
    foo();
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    bail_fn_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    g = function() {
        for (var x in something) {
            log(x);
        }
    };
"#
);

case!(
    bail_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        for (var x in something) {
            log(x);
        }
        something.x = 1;
    }
"#
);

case!(
    bail_across_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        something.x = 1;
    }
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    bail_deep_nonlinear_scopes,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        for (;;) {
            something.x = 1;
        }
    }
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    across_safe_nonlinear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    for (;;) {
        g = log;
    }
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    into_linear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    if (g) {
        for (var x in something) {
            log(x);
        }
    }
"#
);

case!(
    across_linear_scope,
    |cx| passes!(cx),
    r#"
    let something = {};
    if (g) {
        g = log;
    }
    for (var x in something) {
        log(x);
    }
"#
);

case!(
    bail_across_switch_case,
    |cx| passes!(cx),
    r#"
    switch (foo) {
        case 1:
            let something = {};
        default:
            for (var x in something) {
                log(x);
            }
    }
"#
);
