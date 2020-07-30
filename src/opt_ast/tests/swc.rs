case!(basic_empty_if, all_passes, r#"
    if (x) {
        console.log(1);
    } else {
        console.log(2);
        console.log(3);
    }
"#, @r###"
if (x) console.log(1);
else {
    console.log(2);
    console.log(3);
}
"###);

case!(chained_single_stmt_ifs, all_passes, r#"
    if (x) {
        if (y) {
            console.log(1);
        } else if (z) {
            console.log(2);
        }
    // this else should not get attached to the inner if-elseif
    } else {
        console.log(3);
    }
"#, @r###"
if (x) {
    if (y) console.log(1);
    else if (z) console.log(2);
} else console.log(3);
"###);

case!(if_zero_bitor, all_passes, r#"
    if (0 | x) {
        first();
    } else {
        second();
    }
"#, @r###"
if (0 | x) first();
else second();
"###);
