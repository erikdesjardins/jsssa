use swc_common::chain;
use swc_ecma_ast as ast;
use swc_ecma_visit::FoldWith;

use crate::swc_globals;

mod if2cond;
mod merge_vars;
mod resugar_loops;
mod swc;

#[cfg(test)]
mod tests;

pub struct Opt {
    pub minify: bool,
}

#[inline(never)] // for better profiling
pub fn run(g: &swc_globals::Initialized, ast: ast::Program, options: Opt) -> ast::Program {
    let ast = swc::run_passes(g, ast);

    let ast = ast.fold_with(&mut chain!(if2cond::If2Cond, resugar_loops::ResugarLoops));

    let ast = if options.minify {
        ast.fold_with(&mut merge_vars::MergeVars)
    } else {
        ast
    };

    swc::run_passes(g, ast)
}
