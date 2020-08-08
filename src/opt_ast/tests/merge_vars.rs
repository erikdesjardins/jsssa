use crate::opt_ast::merge_vars;

case!(basic, || merge_vars::MergeVars, r#"
    var x;
    var y;
    var z;
    let a;
    let b;
    let c;
    const d;
    const e;
    const f;
"#, @r###"
var x, y, z;
let a, b, c;
const d, e, f;
"###);

case!(basic_values, || merge_vars::MergeVars, r#"
    var x;
    var y = 1;
    var z;
    let a;
    let b = 2;
    let c;
    const d;
    const e = 3;
    const f;
"#, @r###"
var x, y = 1, z;
let a, b = 2, c;
const d, e = 3, f;
"###);

case!(inner_scopes, || merge_vars::MergeVars, r#"
    if (foo) {
        var a;
        var b = 1;
    }
"#, @r###"
if (foo) {
    var a, b = 1;
}
"###);

case!(inner_fn_scopes, || merge_vars::MergeVars, r#"
    function foo() {
        var a;
        var b = 1;
    }
"#, @r###"
function foo() {
    var a, b = 1;
}
"###);

case!(bail_nondecl, || merge_vars::MergeVars, r#"
    var a;
    foo();
    var b = 1;
"#, @r###"
var a;
foo();
var b = 1;
"###);

case!(bail_different_types, || merge_vars::MergeVars, r#"
    var x;
    let y;
    const z;
    var a;
"#, @r###"
var x;
let y;
const z;
var a;
"###);
