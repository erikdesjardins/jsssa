use crate::opt::constant;

case!(
    basic,
    |cx| cx.run::<constant::Prop>("const-prop"),
    r#"
    (typeof (1 + 1 + 1 + 1));
"#
);

case!(
    empty_for,
    |cx| cx.run::<constant::Prop>("const-prop"),
    r#"
    for (var x in {}) bad;
    for (var x of []) bad;
    for (var x in { z: 1 }) good;
    for (var x of [1]) good;
"#
);

case!(
    dead_if,
    |cx| cx.run::<constant::Prop>("const-prop"),
    r#"
    if (true) good;
    else bad;
    if (0) bad;
    else good;
"#
);
