use crate::opt::writeonly;

case!(
    basic,
    |cx| cx.converge::<writeonly::Objects>("writeonly-objects"),
    r#"
    ({}).x = 1;
"#
);

case!(
    basic_var,
    all_passes,
    r#"
    const x = {};
    log();
    x.z = 3;
"#
);

case!(
    basic_bail,
    all_passes,
    r#"
    const x = {};
    log();
    x.z = 3;
    g = function f() {
        x.y;
    }
"#
);

case!(
    bail_escape,
    all_passes,
    r#"
    const x = {};
    log();
    x.z = 3;
    g = function f() {
        n = x;
    };
"#
);

case!(
    bail_other_index,
    all_passes,
    r#"
    const x = {};
    log();
    z[1] = x;
"#
);

case!(
    bail_other_index2,
    all_passes,
    r#"
    const x = {};
    log();
    z[x] = 1;
"#
);

case!(
    bail_not_an_object,
    all_passes,
    r#"
    window.x = 1;
"#
);
