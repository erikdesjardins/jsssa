use std::mem;

use swc_ecma_ast as ast;
use swc_ecma_visit::{Fold, FoldWith};

/// Merges adjacent variable declarations.
pub struct MergeVars;

impl Fold for MergeVars {
    fn fold_stmts(&mut self, stmts: Vec<ast::Stmt>) -> Vec<ast::Stmt> {
        let stmts = stmts.fold_children_with(self);

        let var_to_stmt = |var| ast::Stmt::Decl(ast::Decl::Var(var));

        let mut out = Vec::with_capacity(stmts.len());
        let mut buffered_var = None;
        for stmt in stmts {
            match (stmt, &mut buffered_var) {
                (ast::Stmt::Decl(ast::Decl::Var(cur)), buf @ None) => {
                    // no buffer yet, buffer this decl
                    *buf = Some(cur);
                }
                (ast::Stmt::Decl(ast::Decl::Var(cur)), Some(buf)) if cur.kind == buf.kind => {
                    // same kind, add to buffer
                    buf.decls.extend(cur.decls);
                }
                (ast::Stmt::Decl(ast::Decl::Var(cur)), Some(_)) => {
                    // different kinds, swap into buffer
                    let buffered_var = mem::replace(&mut buffered_var, Some(cur));
                    if let Some(buf) = buffered_var {
                        out.push(var_to_stmt(buf));
                    }
                }
                (stmt, _) => {
                    // not a var decl, flush buffer
                    if let Some(buf) = buffered_var.take() {
                        out.push(var_to_stmt(buf));
                    }
                    out.push(stmt);
                }
            }
        }
        if let Some(buf) = buffered_var {
            out.push(var_to_stmt(buf));
        }
        out
    }
}
