#![feature(box_syntax)]
#![recursion_limit = "128"]
#![allow(clippy::unneeded_field_pattern)]

mod ast2ir;
mod ir;
mod ir2ast;
mod parse;

fn main() {
    let ast = parse::parse(
        r#"
            (function f(x) {
                while (true);
                x = y.bar;
                z.foo = x ? true : 'hi';
                return +[1 || x, { x }, f + 1, ++g];
            });
            f(1), true;
        "#,
    )
    .unwrap();
    let ir = ast2ir::convert(ast);
    println!("{:#?}", ir);
}
