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
extern crate rand;
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
    match parse::parse("1") {
        Ok(ast) => println!("{}", serde_json::to_string(&ast).unwrap()),
        Err(err) => println!("{}", err),
    }
}
