use std::collections::BTreeMap;

use swc_atoms::JsWord;
use swc_common::Span;
use swc_ecma_ast as ast;

use crate::ir;
use crate::ir::scope;
use crate::utils::P;

fn temp_span() -> Span {
    // todo
    unimplemented!()
}

pub fn convert(ir: ir::Block) -> ast::Script {
    // todo perform inlining at this stage? (i.e. scan backwards for all usages)
    let body = convert_block(ir, &scope::Ir::default());
    ast::Script {
        span: temp_span(),
        body,
        shebang: None,
    }
}

pub fn convert_block(block: ir::Block, parent_scope: &scope::Ir) -> Vec<ast::Stmt> {
    let mut scope = parent_scope.clone();

    let ir::Block { children } = block;

    children
        .into_iter()
        .flat_map(|stmt| convert_stmt(stmt, &mut scope))
        .collect()
}

pub fn convert_stmt(stmt: ir::Stmt, scope: &mut scope::Ir) -> Vec<ast::Stmt> {
    unimplemented!()
}
