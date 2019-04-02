#![feature(drain_filter)]
#![warn(clippy::print_stdout)]
#![allow(
    clippy::unneeded_field_pattern,
    clippy::cognitive_complexity,
    clippy::map_clone,
    clippy::single_match
)]

use std::fs;
use std::io;
use std::io::{Read, Write};
use std::time::Instant;

use structopt::StructOpt;

use crate::utils::{NiceError, Time};

mod ast2ir;
mod cli;
mod collections;
mod emit;
mod ir;
mod ir2ast;
mod opt;
mod parse;
mod swc_globals;
mod utils;

#[cfg(test)]
mod tests;

fn main() -> Result<(), NiceError> {
    let cli::Options {
        verbose,
        input,
        output,
        optimize,
        print_ir,
    } = cli::Options::from_args();

    env_logger::Builder::new()
        .filter_level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        })
        .init();

    swc_globals::with(|g| {
        let start = Instant::now();

        let input_string = if input == "-" {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            s
        } else {
            fs::read_to_string(input)?
        };
        log::info!("Done reading @ {}", Time(start.elapsed()));

        let (ast, files) = parse::parse(g, input_string)?;
        log::info!("Done parsing @ {}", Time(start.elapsed()));

        let ir = ast2ir::convert(g, ast);
        log::info!("Done ast2ir @ {}", Time(start.elapsed()));

        let ir = if optimize {
            let ir = opt::run_passes(g, ir);
            log::info!("Done optimization @ {}", Time(start.elapsed()));
            ir
        } else {
            ir
        };

        let output_string = if print_ir {
            let ppr = ir::print(g, &ir);
            log::info!("Done printing @ {}", Time(start.elapsed()));
            ppr
        } else {
            let ast = ir2ast::convert(g, ir);
            log::info!("Done ir2ast @ {}", Time(start.elapsed()));

            let js = emit::emit(g, ast, files)?;
            log::info!("Done emitting @ {}", Time(start.elapsed()));
            js
        };

        if output == "-" {
            io::stdout().write_all(output_string.as_bytes())?;
        } else {
            fs::write(output, output_string)?;
        }
        log::info!("Done writing @ {}", Time(start.elapsed()));

        Ok(())
    })
}
