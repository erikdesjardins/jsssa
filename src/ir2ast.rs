use ast;
use ir;

pub fn convert(ir: ir::Block) -> ast::File {
    // perform inlining at this stage? (i.e. scan backwards for all usages)
    unimplemented!();
}
