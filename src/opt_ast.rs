use swc_ecma_ast as ast;
use swc_ecma_visit::FoldWith;

use crate::swc_globals;

mod if2cond;
mod swc;

#[cfg(test)]
mod tests;

#[inline(never)] // for better profiling
pub fn run(g: &swc_globals::Initialized, ast: ast::Program) -> ast::Program {
    let ast = swc::run_passes(g, ast);

    let ast = ast.fold_with(&mut if2cond::If2Cond);

    swc::run_passes(g, ast)
}
