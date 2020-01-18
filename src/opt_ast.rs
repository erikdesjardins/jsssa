use swc_common::FoldWith;
use swc_ecma_ast as ast;
use swc_ecma_transforms as transforms;

use crate::swc_globals;

#[inline(never)] // for better profiling
pub fn run(_: &swc_globals::Initialized, ast: ast::Program) -> ast::Program {
    let simplified_ast = ast.fold_with(&mut transforms::optimization::simplifier());

    simplified_ast
}
