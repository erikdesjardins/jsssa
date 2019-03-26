#![feature(drain_filter)]
#![allow(clippy::unneeded_field_pattern, clippy::cognitive_complexity)]

use std::io;
use std::io::Read;

mod ast2ir;
mod emit;
mod ir;
mod ir2ast;
mod parse;
mod swc_globals;
mod utils;

#[cfg(test)]
mod tests;

fn main() {
    let mut js = String::new();
    io::stdin().read_to_string(&mut js).unwrap();
    swc_globals::with(|g| {
        let (ast, files) = parse::parse(g, js).unwrap();
        let ir = ast2ir::convert(g, ast);
        println!("----------");
        let ppr = ir::print(g, &ir);
        println!("{}", ppr);
        println!("----------");
        let ast2 = ir2ast::convert(g, ir);
        let js2 = emit::emit(g, ast2, files).unwrap();
        println!("{}", js2);
    });
}
