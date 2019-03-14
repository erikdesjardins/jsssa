use swc_ecma_ast as ast;

use crate::ir;

pub fn convert(ir: ir::Block) -> ast::Script {
    // todo perform inlining at this stage? (i.e. scan backwards for all usages)
    unimplemented!();
}
