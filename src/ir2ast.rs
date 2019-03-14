use swc_ecma_ast as ast;

use crate::ir;

pub fn convert(ir: ir::Block) -> Vec<ast::Stmt> {
    // todo perform inlining at this stage? (i.e. scan backwards for all usages)
    unimplemented!();
}
