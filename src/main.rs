#![feature(plugin)]
#![feature(box_syntax)]
#![recursion_limit = "128"]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![cfg_attr(feature = "clippy", deny(warnings))]
#![cfg_attr(feature = "clippy", allow(unneeded_field_pattern))]
#![cfg_attr(not(feature = "clippy"), allow(unknown_lints))]

extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate if_chain;
#[macro_use]
extern crate mozjs;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

mod ast;
mod ast2ir;
mod ffi;
mod ir;
mod ir2ast;
mod parse;
mod util;

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
    ).unwrap();
    let ir = ast2ir::convert(ast);
    println!("{:#?}", ir);
}
