#![feature(plugin)]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![cfg_attr(feature = "clippy", deny(warnings))]
#![cfg_attr(not(feature = "clippy"), allow(unknown_lints))]
#![feature(box_syntax)]
#![recursion_limit = "128"]

extern crate failure;
#[macro_use]
extern crate failure_derive;
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
                return +[1, { x }, f + 1, ++g];
            });
        "#,
    ).unwrap();
    let ir = ast2ir::convert(ast);
    println!("{:?}", ir);
}
