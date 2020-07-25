use swc_common::chain;
use swc_ecma_ast as ast;
use swc_ecma_transforms as transforms;
use swc_ecma_visit::FoldWith;

use crate::swc_globals;

#[inline(never)] // for better profiling
pub fn run(_: &swc_globals::Initialized, ast: ast::Program) -> ast::Program {
    // ideally we would use the following transform, but it removes assignments to global variables
    // so instead we use the components of it that don't cause problems
    let _ideally_we_would_use_this = transforms::optimization::simplifier(Default::default());

    #[allow(clippy::let_and_return)]
    let simplified_ast = ast.fold_with(&mut chain!(
        transforms::resolver(),
        transforms::optimization::simplify::expr_simplifier(),
        transforms::optimization::simplify::inlining::inlining(Default::default()),
        transforms::optimization::simplify::dead_branch_remover(),
        //transforms::optimization::simplify::dce::dce(Default::default())
    ));

    simplified_ast
}
