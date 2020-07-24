#![warn(clippy::dbg_macro, clippy::print_stdout)]
#![allow(
    clippy::unneeded_field_pattern,
    clippy::cognitive_complexity,
    clippy::option_map_unit_fn,
    clippy::map_clone,
    clippy::match_bool,
    clippy::single_match
)]
#![allow(unstable_name_collisions)]

use std::fs;
use std::io;
use std::io::{Read, Write};
use std::time::Instant;

use crate::err::NiceError;
use crate::utils::Time;

mod anal;
mod ast2ir;
mod cli;
mod collections;
mod emit;
mod err;
mod ir;
mod ir2ast;
mod opt;
mod opt_ast;
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
        minify,
        optimize: _,
        opt_ir,
        opt_inline_ssa,
        opt_ast,
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

        let ir = if opt_ir {
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
            let ast = ir2ast::convert(
                g,
                ir,
                ir2ast::Opt {
                    inline: opt_inline_ssa,
                    minify,
                },
            );
            log::info!("Done ir2ast @ {}", Time(start.elapsed()));

            let ast = if opt_ast {
                let ast = opt_ast::run(g, ast);
                log::info!("Done ast optimization @ {}", Time(start.elapsed()));
                ast
            } else {
                ast
            };

            let js = emit::emit(g, ast, files, emit::Opt { minify })?;
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
