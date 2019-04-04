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
