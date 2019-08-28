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
"#,
@r###"
<dead> = "x"
_val = 1
<dead> = _val
"###);

case!(
    basic_var,
    |cx| passes!(cx),
    r#"
    const x = {};
    log();
    x.z = 3;
"#,
@r###"
_fun = <global log>
<dead> = _fun()
<dead> = "z"
<dead> = 3
"###);

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
"#,
@r###"
x = {  }
_fun = <global log>
<dead> = _fun()
_prp = "z"
_val = 3
x[_prp] <- _val
_val$1 = <function>:
    _prp$1 = "y"
    <dead> = x[_prp$1]
<global g> <- _val$1
"###);

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
"#,
@r###"
x = {  }
_fun = <global log>
<dead> = _fun()
_prp = "z"
_val = 3
x[_prp] <- _val
_val$1 = <function>:
    <global n> <- x
<global g> <- _val$1
"###);

case!(
    bail_other_index,
    |cx| passes!(cx),
    r#"
    const x = {};
    log();
    z[1] = x;
"#,
@r###"
x = {  }
_fun = <global log>
<dead> = _fun()
_obj = <global z>
_prp = 1
_obj[_prp] <- x
"###);

case!(
    bail_other_index2,
    |cx| passes!(cx),
    r#"
    const x = {};
    log();
    z[x] = 1;
"#,
@r###"
x = {  }
_fun = <global log>
<dead> = _fun()
_obj = <global z>
_val = 1
_obj[x] <- _val
"###);

case!(
    bail_not_an_object,
    |cx| passes!(cx),
    r#"
    window.x = 1;
"#,
@r###"
_obj = <global window>
_prp = "x"
_val = 1
_obj[_prp] <- _val
"###);
