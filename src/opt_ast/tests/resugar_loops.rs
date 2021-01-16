use crate::opt_ast::resugar_loops;

case!(basic, || resugar_loops::ResugarLoops, r#"
    for(;;){
        if (a < A.length);
        else break;
        a += 1;
    }
"#, @r###"
for(; a < A.length;){
    a += 1;
}
"###);

case!(nested, || resugar_loops::ResugarLoops, r#"
    for(;;){
        if (a < A.length);
        else break;
        a += 1;

        for(;;){
            if (b < B.length);
            else break;
            b += 1;
        }
    }
"#, @r###"
for(; a < A.length;){
    a += 1;
    for(; b < B.length;){
        b += 1;
    }
}
"###);

case!(bail_existing_test, || resugar_loops::ResugarLoops, r#"
    for(;b < 1;){
        if (a < A.length);
        else break;
        a += 1;
    }
"#, @r###"
for(; b < 1;){
    if (a < A.length) ;
    else break;
    a += 1;
}
"###);

case!(bail_labelled, || resugar_loops::ResugarLoops, r#"
    outer: for (;;) {
        for(;;){
            if (a < A.length);
            else break outer;
            a += 1;
        }
    }
"#, @r###"
outer: for(;;){
    for(;;){
        if (a < A.length) ;
        else break outer;
        a += 1;
    }
}
"###);

case!(bail_consequent, || resugar_loops::ResugarLoops, r#"
    for(;;){
        if (a < A.length) log();
        else break;
        a += 1;
    }
"#, @r###"
for(;;){
    if (a < A.length) log();
    else break;
    a += 1;
}
"###);

case!(bail_alternate, || resugar_loops::ResugarLoops, r#"
    for(;;){
        if (a < A.length) ;
        else log();
        a += 1;
    }
"#, @r###"
for(;;){
    if (a < A.length) ;
    else log();
    a += 1;
}
"###);
