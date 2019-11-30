use std::collections::{HashMap, HashSet};
use std::iter;

use crate::collections::ZeroOneMany::{self, Many, One};
use crate::ir;
use crate::ir::traverse::{Folder, RunVisitor, ScopeTy, Visitor};

/// Scalar replacement of aggregates (objects).
///
/// May profit from multiple passes.
/// Does not profit from DCE running first; may create opportunities for DCE.
/// May create opportunities for mut-to-ssa downleveling.
/// May create opportunities for read forwarding.
#[derive(Debug, Default)]
pub struct Replace {
    objects_to_replace: HashMap<ir::WeakRef<ir::Ssa>, HashMap<String, ir::Ref<ir::Mut>>>,
    known_strings: HashMap<ir::WeakRef<ir::Ssa>, String>,
}

#[derive(Debug, Default)]
struct CollectObjInfo<'a> {
    known_objs: HashMap<&'a ir::Ref<ir::Ssa>, State<'a>>,
    known_strings: HashMap<&'a ir::Ref<ir::Ssa>, &'a str>,
    last_use_was_safe: bool,
}

#[derive(Debug)]
enum State<'a> {
    NoObjYet(HashSet<&'a str>),
    HasProps(HashSet<&'a str>),
    Invalid,
}

fn is_safe_prop(prop: &str) -> bool {
    const OBJ_PROTO_PROPS: [&str; 12] = [
        "constructor",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toLocaleString",
        "toString",
        "valueOf",
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
        "__proto__",
    ];
    !OBJ_PROTO_PROPS.contains(&prop)
}

impl<'a> CollectObjInfo<'a> {
    fn declare_simple_object(
        &mut self,
        obj: &'a ir::Ref<ir::Ssa>,
        props: impl IntoIterator<Item = &'a ir::Ref<ir::Ssa>>,
    ) {
        let known_safe_props = props
            .into_iter()
            .map(|prop| match self.known_strings.get(prop) {
                Some(prop) if is_safe_prop(prop) => Ok(*prop),
                _ => Err(()),
            })
            .collect::<Result<_, _>>();
        match (self.known_objs.get_mut(obj), known_safe_props) {
            (None, Ok(props)) => {
                self.known_objs.insert(obj, State::HasProps(props));
            }
            (Some(State::NoObjYet(seen_props)), Ok(ref mut props)) => {
                let all_props = seen_props.drain().chain(props.drain()).collect();
                self.known_objs.insert(obj, State::HasProps(all_props));
            }
            (Some(State::HasProps(_)), _) => unreachable!("multiple ssa defns"),
            (Some(State::Invalid), _) => { /* already invalid */ }
            (_, Err(())) => {
                self.known_objs.insert(obj, State::Invalid);
            }
        }
    }

    fn access_prop(&mut self, obj: &'a ir::Ref<ir::Ssa>, prop: &'a ir::Ref<ir::Ssa>) {
        let known_safe_prop = match self.known_strings.get(prop) {
            Some(prop) if is_safe_prop(prop) => Some(*prop),
            _ => None,
        };
        match (self.known_objs.get_mut(obj), known_safe_prop) {
            (None, Some(prop)) => {
                self.known_objs
                    .insert(obj, State::NoObjYet(iter::once(prop).collect()));
            }
            (Some(State::NoObjYet(props)), Some(prop)) => {
                props.insert(prop);
            }
            (Some(State::HasProps(props)), Some(prop)) => {
                props.insert(prop);
            }
            (Some(State::Invalid), _) => { /* already invalid */ }
            (_, None) => {
                self.known_objs.insert(obj, State::Invalid);
            }
        }
    }
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
                    let all_simple_props = props
                        .iter()
                        .map(|(kind, prop, _val)| match kind {
                            ir::PropKind::Simple => Ok(prop),
                            ir::PropKind::Get | ir::PropKind::Set => Err(()),
                        })
                        .collect::<Result<Vec<_>, _>>();
                    match all_simple_props {
                        Ok(simple_props) => {
                            self.declare_simple_object(target, simple_props);
                        }
                        Err(()) => {
                            self.known_objs.insert(target, State::Invalid);
                        }
                    }
                }
                ir::Expr::ReadMember { obj, prop } => {
                    self.last_use_was_safe = true;
                    self.access_prop(obj, prop);
                    self.known_objs.insert(prop, State::Invalid);
                }
                _ => {}
            },
            ir::Stmt::WriteMember { obj, prop, val } => {
                self.last_use_was_safe = true;
                self.access_prop(obj, prop);
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
                    State::NoObjYet(_) | State::Invalid => None,
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
                Some(prop_vars) => {
                    let mut prop_vars = prop_vars.clone();
                    let undef_ref = ir::Ref::new("_mis");
                    let undef = ir::Stmt::Expr {
                        target: undef_ref.clone(),
                        expr: ir::Expr::Undefined,
                    };
                    let props_with_values = props
                        .into_iter()
                        .map(|(kind, prop, val)| {
                            match (
                                kind,
                                self.known_strings
                                    .get(&prop.weak())
                                    .and_then(|name| prop_vars.remove(name)),
                            ) {
                                (ir::PropKind::Simple, Some(prop_var)) => {
                                    ir::Stmt::DeclareMutable {
                                        target: prop_var,
                                        val,
                                    }
                                }
                                (ir::PropKind::Get, _) | (ir::PropKind::Set, _) | (_, None) => {
                                    unreachable!("bad prop kind or unknown name")
                                }
                            }
                        })
                        .collect::<Vec<_>>();
                    let props_without_values =
                        prop_vars
                            .into_iter()
                            .map(|(_, prop_var)| ir::Stmt::DeclareMutable {
                                target: prop_var,
                                val: undef_ref.clone(),
                            });
                    Many(
                        iter::once(undef)
                            .chain(props_with_values)
                            .chain(props_without_values)
                            .collect(),
                    )
                }
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
