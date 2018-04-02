#![feature(plugin)]
#![plugin(clippy)]
#![deny(warnings)]

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
