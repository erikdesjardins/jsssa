use swc_ecma_ast as ast;
use swc_ecma_visit::FoldWith;

use crate::swc_globals;

mod if2cond;
mod merge_vars;
mod swc;

#[cfg(test)]
mod tests;

pub struct Opt {
    pub minify: bool,
}

#[inline(never)] // for better profiling
pub fn run(g: &swc_globals::Initialized, ast: ast::Program, options: Opt) -> ast::Program {
    let ast = swc::run_passes(g, ast);

    let ast = ast.fold_with(&mut if2cond::If2Cond);

    let ast = if options.minify {
        ast.fold_with(&mut merge_vars::MergeVars)
    } else {
        ast
    };

    swc::run_passes(g, ast)
}
