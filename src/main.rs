#![feature(drain_filter)]
#![allow(clippy::unneeded_field_pattern)]

use std::io;
use std::io::Read;

mod ast2ir;
mod ir;
mod ir2ast;
mod parse;
mod utils;

#[cfg(test)]
mod tests;

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    parse::parse(s, |ast| {
        let ir = ast2ir::convert(ast.unwrap());
        println!("{:#?}", ir);
    });
}
