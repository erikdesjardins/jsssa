use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::ir;
use crate::ir::traverse::{visit_with, Folder, ScopeTy};

/// Converts read-only mutable vars to SSA, and removes write-only mutable vars.
///
/// Does not profit from multiple passes.
/// Profits from DCE running first, to remove unused reads; may also create opportunities for DCE.
/// May create opportunities for read forwarding.
#[derive(Default)]
pub struct Mut2Ssa {
    mut_vars_to_replace: HashMap<ir::WeakRef<ir::Mut>, What>,
}

enum What {
    Convert(ir::Ref<ir::Ssa>),
    Remove,
}

impl Folder for Mut2Ssa {
    type Output = Option<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            #[derive(PartialEq)]
            enum Saw {
                Read,
                Write,
                Both,
            }

            let mut potential_replacements = HashMap::new();

            visit_with(&block, |stmt: &ir::Stmt| {
                let read_or_write = match stmt {
                    ir::Stmt::Expr {
                        target: _,
                        expr: ir::Expr::ReadMutable { source },
                    } => Some((source, Saw::Read)),
                    ir::Stmt::WriteMutable { target, val: _ } => Some((target, Saw::Write)),
                    _ => None,
                };
                if let Some((ref_, saw)) = read_or_write {
                    match potential_replacements.entry(ref_) {
                        Entry::Vacant(e) => {
                            e.insert(saw);
                        }
                        Entry::Occupied(ref mut e) if e.get() != &saw => {
                            e.insert(Saw::Both);
                        }
                        Entry::Occupied(_) => {}
                    }
                }
            });

            self.mut_vars_to_replace = potential_replacements
                .into_iter()
                .flat_map(|(ref_, saw)| match saw {
                    Saw::Read => Some((ref_.weak(), What::Convert(ir::Ref::new(ref_.name_hint())))),
                    Saw::Write => Some((ref_.weak(), What::Remove)),
                    Saw::Both => None,
                })
                .collect();
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                ref target,
                expr: ir::Expr::ReadMutable { ref source },
            } => match self.mut_vars_to_replace.get(&source.weak()) {
                Some(What::Convert(ssa_ref)) => Some(ir::Stmt::Expr {
                    target: target.clone(),
                    expr: ir::Expr::Read {
                        source: ssa_ref.clone(),
                    },
                }),
                Some(What::Remove) => unreachable!("removing mut read: {:?}", source),
                None => Some(stmt),
            },
            ir::Stmt::DeclareMutable {
                ref target,
                ref val,
            } => match self.mut_vars_to_replace.get(&target.weak()) {
                Some(What::Convert(ssa_ref)) => Some(ir::Stmt::Expr {
                    target: ssa_ref.clone(),
                    expr: ir::Expr::Read {
                        source: val.clone(),
                    },
                }),
                Some(What::Remove) => None,
                None => Some(stmt),
            },
            ir::Stmt::WriteMutable { ref target, val: _ } => {
                match self.mut_vars_to_replace.get(&target.weak()) {
                    Some(What::Convert(_)) => unreachable!("converting mut write: {:?}", target),
                    Some(What::Remove) => None,
                    None => Some(stmt),
                }
            }
            _ => Some(stmt),
        }
    }
}
