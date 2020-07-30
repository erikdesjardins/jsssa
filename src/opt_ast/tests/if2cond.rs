use crate::opt_ast::if2cond;

case!(basic, || if2cond::If2Cond, r#"
    var x;
    if (foo) x = 1;
    else x = 2;
"#, @r###"
var x;
x = foo ? 1 : 2;
"###);

case!(bail_different_var, || if2cond::If2Cond, r#"
    var x;
    if (foo) x = 1;
    else y = 2;
"#, @r###"
var x;
if (foo) x = 1;
else y = 2;
"###);

case!(bail_different_var_declared, || if2cond::If2Cond, r#"
    var x, y;
    if (foo) x = 1;
    else y = 2;
"#, @r###"
var x, y;
if (foo) x = 1;
else y = 2;
"###);

case!(bail_multiple_exprs, || if2cond::If2Cond, r#"
    var x;
    if (foo) { x = 1; foo(); }
    else x = 2;
"#, @r###"
var x;
if (foo) {
    x = 1;
    foo();
} else x = 2;
"###);

case!(bail_multiple_exprs2, || if2cond::If2Cond, r#"
    var x;
    if (foo) x = 1;
    else { x = 2; foo(); }
"#, @r###"
var x;
if (foo) x = 1;
else {
    x = 2;
    foo();
}
"###);

case!(bail_comma, || if2cond::If2Cond, r#"
    var x;
    if (foo) (x = 1, foo());
    else x = 2;
"#, @r###"
    var x;
    if (foo) x = 1, foo();
    else x = 2;
"###);

case!(bail_comma2, || if2cond::If2Cond, r#"
    var x;
    if (foo) x = 1;
    else (x = 2, foo());
"#, @r###"
    var x;
    if (foo) x = 1;
    else x = 2, foo();
"###);

case!(bail_special_op, || if2cond::If2Cond, r#"
    var x;
    if (foo) x += 1;
    else x = 2;
"#, @r###"
var x;
if (foo) x += 1;
else x = 2;
"###);

case!(bail_special_op2, || if2cond::If2Cond, r#"
    var x;
    if (foo) x = 1;
    else x += 2;
"#, @r###"
var x;
if (foo) x = 1;
else x += 2;
"###);
