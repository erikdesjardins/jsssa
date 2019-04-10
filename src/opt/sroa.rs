use std::collections::{HashMap, HashSet};

use crate::collections::ZeroOneMany::{self, Many, One};
use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Scalar replacement of aggregates (objects).
///
/// May profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for mut-to-ssa downleveling.
/// May create opportunities for read forwarding.
#[derive(Default)]
pub struct Replace {
    objects_to_replace: HashMap<ir::WeakRef<ir::Ssa>, HashMap<String, ir::Ref<ir::Mut>>>,
    known_strings: HashMap<ir::WeakRef<ir::Ssa>, String>,
}

#[derive(Default)]
struct CollectObjInfo<'a> {
    known_objs: HashMap<&'a ir::Ref<ir::Ssa>, State<'a>>,
    known_strings: HashMap<&'a ir::Ref<ir::Ssa>, &'a str>,
    last_use_was_safe: bool,
}

#[derive(Debug)]
enum State<'a> {
    HasProps(HashSet<&'a str>),
    Invalid,
}

impl<'a> Visitor<'a> for CollectObjInfo<'a> {
    fn visit(&mut self, stmt: &'a ir::Stmt) {
        self.last_use_was_safe = false;

        match stmt {
            ir::Stmt::Expr { target, expr } => match expr {
                ir::Expr::String { value } => {
                    self.known_strings.insert(target, &value);
                }
                ir::Expr::Object { props } => {
                    let safe_props = props
                        .iter()
                        .map(|(kind, prop, _val)| match kind {
                            ir::PropKind::Simple => match self.known_strings.get(prop) {
                                Some(prop) => Ok(*prop),
                                None => Err(()),
                            },
                            ir::PropKind::Get | ir::PropKind::Set => {
                                unimplemented!("getter/setter props not handled")
                            }
                        })
                        .collect::<Result<_, _>>();
                    match safe_props {
                        Ok(props) => {
                            // don't overwrite if already invalidated
                            self.known_objs
                                .entry(target)
                                .or_insert(State::HasProps(props));
                        }
                        Err(()) => {
                            self.known_objs.insert(target, State::Invalid);
                        }
                    }
                }
                ir::Expr::ReadMember { obj, prop } => {
                    self.last_use_was_safe = true;
                    match (self.known_objs.get(obj), self.known_strings.get(prop)) {
                        (Some(State::HasProps(props)), Some(prop)) if props.contains(prop) => {
                            // known object, known prop: good
                        }
                        _ => {
                            self.known_objs.insert(obj, State::Invalid);
                        }
                    }
                    self.known_objs.insert(prop, State::Invalid);
                }
                _ => {}
            },
            ir::Stmt::WriteMember { obj, prop, val } => {
                self.last_use_was_safe = true;
                match (self.known_objs.get(obj), self.known_strings.get(prop)) {
                    (Some(State::HasProps(props)), Some(prop)) if props.contains(prop) => {
                        // known object, known prop: good
                    }
                    _ => {
                        self.known_objs.insert(obj, State::Invalid);
                    }
                }
                self.known_objs.insert(prop, State::Invalid);
                self.known_objs.insert(val, State::Invalid);
            }
            _ => {}
        }
    }

    fn visit_ref_use(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
        if !self.last_use_was_safe {
            self.known_objs.insert(ref_, State::Invalid);
        }
    }
}

impl Folder for Replace {
    type Output = ZeroOneMany<ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut collector = CollectObjInfo::default();
            collector.run_visitor(&block);
            self.objects_to_replace = collector
                .known_objs
                .into_iter()
                .filter_map(|(obj, state)| match state {
                    State::HasProps(props) => {
                        let prop_vars = props
                            .into_iter()
                            .map(|prop| (prop.to_string(), ir::Ref::new(prop)))
                            .collect();
                        Some((obj.weak(), prop_vars))
                    }
                    State::Invalid => None,
                })
                .collect();
            self.known_strings = collector
                .known_strings
                .into_iter()
                .map(|(ref_, str)| (ref_.weak(), str.to_string()))
                .collect();
        }

        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output {
        match stmt {
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::Object { props },
            } => match self.objects_to_replace.get(&target.weak()) {
                Some(prop_vars) => Many(
                    props
                        .into_iter()
                        .map(|(kind, prop, val)| {
                            match (
                                kind,
                                self.known_strings
                                    .get(&prop.weak())
                                    .and_then(|name| prop_vars.get(name)),
                            ) {
                                (ir::PropKind::Simple, Some(prop_var)) => {
                                    ir::Stmt::DeclareMutable {
                                        target: prop_var.clone(),
                                        val,
                                    }
                                }
                                (ir::PropKind::Get, _) | (ir::PropKind::Set, _) | (_, None) => {
                                    unreachable!("bad prop kind or unknown name")
                                }
                            }
                        })
                        .collect(),
                ),
                None => One(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Object { props },
                }),
            },
            ir::Stmt::Expr {
                target,
                expr: ir::Expr::ReadMember { obj, prop },
            } => match self.objects_to_replace.get(&obj.weak()) {
                Some(prop_vars) => {
                    // todo store a map from Ref<Ssa> prop refs to Ref<Mut> to avoid double lookup
                    let prop_var = match self
                        .known_strings
                        .get(&prop.weak())
                        .and_then(|name| prop_vars.get(name))
                    {
                        Some(prop_var) => prop_var.clone(),
                        None => unreachable!("unknown prop"),
                    };
                    One(ir::Stmt::Expr {
                        target,
                        expr: ir::Expr::ReadMutable { source: prop_var },
                    })
                }
                None => One(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::ReadMember { obj, prop },
                }),
            },
            ir::Stmt::WriteMember { obj, prop, val } => {
                match self.objects_to_replace.get(&obj.weak()) {
                    Some(prop_vars) => {
                        let prop_var = match self
                            .known_strings
                            .get(&prop.weak())
                            .and_then(|name| prop_vars.get(name))
                        {
                            Some(prop_var) => prop_var.clone(),
                            None => unreachable!("unknown prop"),
                        };
                        One(ir::Stmt::WriteMutable {
                            target: prop_var,
                            val,
                        })
                    }
                    None => One(ir::Stmt::WriteMember { obj, prop, val }),
                }
            }
            _ => One(stmt),
        }
    }
}
