use crate::opt::dce;
use crate::opt::forward;
use crate::opt::mut2ssa;
use crate::opt::writeonly;

macro_rules! passes {
    ( $cx:ident ) => {
        $cx.run::<mut2ssa::Mut2Ssa>("mut2ssa")
            .run::<forward::Reads>("forward-reads-redundancy")
            .converge::<dce::Dce>("dce-forwarded-reads")
            .converge::<writeonly::Objects>("writeonly-objects")
    };
}

case!(
    basic,
    |cx| cx.converge::<writeonly::Objects>("writeonly-objects"),
    r#"
    ({}).x = 1;
"#
);

case!(
    basic_var,
    |cx| passes!(cx),
    r#"
    const x = {};
    log();
    x.z = 3;
"#
);

case!(
    basic_bail,
    |cx| passes!(cx),
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
    |cx| passes!(cx),
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
    |cx| passes!(cx),
    r#"
    const x = {};
    log();
    z[1] = x;
"#
);

case!(
    bail_other_index2,
    |cx| passes!(cx),
    r#"
    const x = {};
    log();
    z[x] = 1;
"#
);

case!(
    bail_not_an_object,
    |cx| passes!(cx),
    r#"
    window.x = 1;
"#
);
