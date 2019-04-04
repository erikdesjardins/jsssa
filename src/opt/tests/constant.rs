use crate::opt::constant;

case!(
    basic,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    (typeof (1 + 1 + 1 + 1));
"#
);

case!(
    basic_bail,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    (typeof (1 + 1 + null + 1));
"#
);

case!(
    empty_for,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    for (var x in {}) bad;
    for (var x of []) bad;
    for (var x in { z: 1 }) good;
    for (var x of [1]) good;
"#
);

case!(
    dead_if,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    if (true) good;
    else bad;
    if (0) bad;
    else good;
"#
);

case!(
    nan_and_undefined_magic_globals,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    g1 = NaN;
    g2 = undefined;
    {
        let NaN = 1;
        let undefined = 2;
        g3 = NaN;
        g4 = undefined;
    }
"#
);
