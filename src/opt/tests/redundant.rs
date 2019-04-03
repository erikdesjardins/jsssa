use crate::opt::redundant;

case!(
    basic_read_to_read,
    |cx| cx.converge::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    foo();
    let x = something;
    let y = something;
"#
);

case!(
    basic_write_to_read,
    |cx| cx.converge::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    foo();
    something = x;
    let y = something;
"#
);

case!(
    basic_write_to_write,
    |cx| cx.converge::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    foo();
    something = x;
    something = y;
"#
);

case!(
    write_to_write_decl,
    |cx| cx.converge::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let something = 1;
    something = x;
    // implicit void init
    var y = 2;
"#
);

case!(
    invalidate_inner_scope_writes,
    |cx| cx.converge::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    let bar = 2;
    let not_written = 2;
    // every ref should be forwarded except the two at the bottom
    if (something) {
        foo;
        foo = 3;
        bar;
        if (something2) {
            bar = 4;
        }
        foo;
        not_written;
    }
    foo; // do not forward
    bar; // do not forward
    not_written;
"#
);

case!(
    many_writes,
    |cx| cx.converge::<redundant::LoadStore>("redundant-load-store"),
    r#"
    let foo = 1;
    foo = 2;
    foo = 3;
    foo = 4;
    foo = 5;
"#
);