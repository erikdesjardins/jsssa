#![feature(plugin)]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![cfg_attr(feature="clippy", deny(warnings))]
#![cfg_attr(not(feature="clippy"), allow(unknown_lints))]

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
mod ffi;
mod parse;

fn main() {
    match parse::parse("1") {
        Ok(ast) => println!("{:?}", ast),
        Err(err) => println!("{}", err),
    }
}
