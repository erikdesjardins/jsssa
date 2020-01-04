use crate::opt::constant;

case!(
    basic,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    (typeof (1 + 1 + 1 + 1));
"#,
@r###"
<dead> = 1
<dead> = 1
<dead> = 2
<dead> = 1
<dead> = 3
<dead> = 1
<dead> = 4
<dead> = "number"
"###);

case!(
    basic_bail,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    (typeof (1 + 1 + null + 1));
"#,
@r###"
<dead> = 1
<dead> = 1
_lhs = 2
_rhs = <null>
_lhs$1 = _lhs + _rhs
_rhs$1 = 1
_una = _lhs$1 + _rhs$1
<dead> = <typeof> _una
"###);

case!(
    dead_if,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    if (true) good;
    else bad;
    if (0) bad;
    else good;
"#,
@r###"
<dead> = true
<dead> = <global good>
<dead> = 0
<dead> = <global good>
"###);

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
"#,
@r###"
_val = +NaN
<global g1> <- _val
<dead> = _val
_val$1 = <void>
<global g2> <- _val$1
<dead> = _val$1
_ini = 1
NaN <= _ini
_ini$1 = 2
undefined <= _ini$1
_val$2 = *NaN
<global g3> <- _val$2
<dead> = _val$2
_val$3 = *undefined
<global g4> <- _val$3
<dead> = _val$3
"###);

case!(
    precompute,
    all_passes,
    r#"
    x = 1 + 1 + 1 + 1;
    y = "foo" + " " + "bar";
"#,
@r###"
_val = 4
<global x> <- _val
_val$1 = "foo bar"
<global y> <- _val$1
"###);

case!(
    string_eq,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    "foo" == "bar";
    "foo" != "bar";
"#,
@r###"
<dead> = "foo"
<dead> = "bar"
<dead> = false
<dead> = "foo"
<dead> = "bar"
<dead> = true
"###);

case!(
    not,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    !void 0
    ![]
"#,
@r###"
<dead> = 0
<dead> = <void>
<dead> = true
<dead> = []
<dead> = false
"###);

case!(
    string_length,
    |cx| cx.run::<constant::ConstProp>("const-prop"),
    r#"
    'foo'.length
    'A\uD87E\uDC04Z'.length
"#,
@r###"
<dead> = "foo"
<dead> = "length"
<dead> = 3
<dead> = "A\u{d87e}\u{dc04}Z"
<dead> = "length"
<dead> = 4
"###);
