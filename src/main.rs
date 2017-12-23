#![feature(plugin)]
#![plugin(clippy)]

extern crate rand;

use std::env;
use std::fs::File;
use std::process::{Command, Stdio};
use std::io::Write;
use rand::Rng;

static ACORN_BIN: &[u8] = include_bytes!("../vendor/acorn.js");

fn run_acorn(js: &str) -> String {
    let mut path = env::temp_dir();
    let random_name: String = rand::thread_rng()
        .gen_ascii_chars()
        .take(32)
        .collect();
    path.push(random_name);

    File::create(&path).unwrap()
        .write_all(ACORN_BIN).unwrap();

    let mut child = Command::new("node")
        .arg(&path)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    write!(child.stdin.as_mut().unwrap(), "{}", js).unwrap();

    let out = child
        .wait_with_output().unwrap()
        .stdout;

    String::from_utf8(out).unwrap()
}

fn main() {
    let ast = run_acorn("1");

    println!("{}", ast);
}
