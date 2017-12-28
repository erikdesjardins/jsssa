#![feature(plugin)]
#![plugin(clippy)]
#![deny(warnings)]

extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

mod ast;
mod parse;

fn main() {
    println!("{:?}", parse::parse("1"));
}
