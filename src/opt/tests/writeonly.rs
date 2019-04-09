use crate::opt::dce;
use crate::opt::forward;
use crate::opt::mut2ssa;
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
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads")
        .converge::<dce::Dce>("dce")
        .converge::<writeonly::Objects>("writeonly-objects"),
    r#"
    const x = {};
    log();
    x.z = 3;
"#
);

case!(
    basic_bail,
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads")
        .converge::<dce::Dce>("dce")
        .converge::<writeonly::Objects>("writeonly-objects"),
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
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads")
        .converge::<dce::Dce>("dce")
        .converge::<writeonly::Objects>("writeonly-objects"),
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
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads")
        .converge::<dce::Dce>("dce")
        .converge::<writeonly::Objects>("writeonly-objects"),
    r#"
    const x = {};
    log();
    z[1] = x;
"#
);

case!(
    bail_other_index2,
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads")
        .converge::<dce::Dce>("dce")
        .converge::<writeonly::Objects>("writeonly-objects"),
    r#"
    const x = {};
    log();
    z[x] = 1;
"#
);

case!(
    bail_not_an_object,
    |cx| cx
        .run::<mut2ssa::Mut2Ssa>("mut2ssa")
        .run::<forward::Reads>("forward-reads")
        .converge::<dce::Dce>("dce")
        .converge::<writeonly::Objects>("writeonly-objects"),
    r#"
    window.x = 1;
"#
);
