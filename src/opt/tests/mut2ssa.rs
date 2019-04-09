use crate::opt::mut2ssa;

case!(
    basic,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    let x = 1;
    x = 2;
    x = 3;

    let y = 10;
    log(y);
    log(y + 1);
"#
);

case!(
    basic_bail,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    let x = 1;
    x = 2;
    x = 3;
    if (foo) log(x);

    let y = 10;
    log(y);
    log(y + 1);
    if (bar) (function() {
        y = 5;
    })();
"#
);

case!(
    downleveling,
    all_passes,
    r#"
    let x = 1;
    x = 2;
    x = 3;

    let y = 10;
    log(y);
    log(y + 1);
"#
);

case!(
    downleveling_bail,
    all_passes,
    r#"
    let x = 1;
    x = 2;
    x = 3;
    if (foo) log(x);

    let y = 10;
    log(y);
    log(y + 1);
    if (bar) (function() {
        y = 5;
    })();
"#
);

case!(
    no_time_travel,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    x;
    let x = 1;
"#
);

case!(
    no_cross_case,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    switch (foo) {
        case 0:
            let x = 1;
        default:
            g = function() { return x };
    }
"#
);

case!(
    remove_writeonly_cross_case,
    |cx| cx.run::<mut2ssa::Mut2Ssa>("mut2ssa"),
    r#"
    switch (foo) {
        case 0:
            let x = 1;
        default:
            g = function() { x = 2 };
    }
"#
);
