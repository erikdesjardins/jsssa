use std::collections::{HashMap, HashSet};
use std::iter;

use crate::anl;
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
struct CollectObjDeclInfo<'a> {
    fns_without_this: HashSet<&'a ir::Ref<ir::Ssa>>,
    known_strings: HashMap<&'a ir::Ref<ir::Ssa>, &'a str>,
    known_objs: HashMap<&'a ir::Ref<ir::Ssa>, HashMap<&'a str, PropInfo>>,
}

#[derive(Debug)]
struct PropInfo {
    safe_for_call: bool,
    saw_call: bool,
}

#[derive(Debug, Default)]
struct CollectObjAccessInfo<'a> {
    fns_without_this: HashSet<&'a ir::Ref<ir::Ssa>>,
    known_strings: HashMap<&'a ir::Ref<ir::Ssa>, &'a str>,
    known_objs: HashMap<&'a ir::Ref<ir::Ssa>, HashMap<&'a str, PropInfo>>,
    last_use_was_safe: bool,
}

enum Access {
    Read,
    Call,
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

impl<'a> Visitor<'a> for CollectObjDeclInfo<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            self.fns_without_this = anl::fns::without_this(&block);
        }

        enter(self, block)
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        match stmt {
            ir::Stmt::Expr { target, expr } => match expr {
                ir::Expr::String { value } => {
                    self.known_strings.insert(target, &value);
                }
                ir::Expr::Object { props } => {
                    let all_simple_safe_props = props
                        .iter()
                        .map(|(kind, prop, val)| match kind {
                            ir::PropKind::Simple => match self.known_strings.get(prop) {
                                Some(prop) if is_safe_prop(prop) => Ok((
                                    *prop,
                                    PropInfo {
                                        safe_for_call: self.fns_without_this.contains(val),
                                        saw_call: false,
                                    },
                                )),
                                _ => Err(()),
                            },
                            ir::PropKind::Get | ir::PropKind::Set => Err(()),
                        })
                        .collect::<Result<_, _>>();
                    if let Ok(props) = all_simple_safe_props {
                        self.known_objs.insert(target, props);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl<'a> CollectObjAccessInfo<'a> {
    fn access_prop(&mut self, kind: Access, obj: &'a ir::Ref<ir::Ssa>, prop: &'a ir::Ref<ir::Ssa>) {
        if let Some(props) = self.known_objs.get_mut(obj) {
            let known_safe_prop = match self.known_strings.get(prop) {
                Some(prop) if is_safe_prop(prop) => Some(*prop),
                _ => None,
            };
            match known_safe_prop {
                Some(prop) => {
                    let entry = props.entry(prop).or_insert(PropInfo {
                        safe_for_call: true,
                        saw_call: false,
                    });
                    match kind {
                        Access::Read => {
                            // all reads are safe
                        }
                        Access::Call => {
                            entry.saw_call = true;
                        }
                    }
                }
                None => {
                    self.known_objs.remove(obj);
                }
            }
        }
    }

    fn write_prop(
        &mut self,
        obj: &'a ir::Ref<ir::Ssa>,
        prop: &'a ir::Ref<ir::Ssa>,
        val: &'a ir::Ref<ir::Ssa>,
    ) {
        if let Some(props) = self.known_objs.get_mut(obj) {
            let known_safe_prop = match self.known_strings.get(prop) {
                Some(prop) if is_safe_prop(prop) => Some(*prop),
                _ => None,
            };
            match known_safe_prop {
                Some(prop) => {
                    let entry = props.entry(prop).or_insert(PropInfo {
                        safe_for_call: true,
                        saw_call: false,
                    });
                    entry.safe_for_call &= self.fns_without_this.contains(val);
                }
                None => {
                    self.known_objs.remove(obj);
                }
            }
        }
    }
}

impl<'a> Visitor<'a> for CollectObjAccessInfo<'a> {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        if let ScopeTy::Toplevel = ty {
            let mut collector = CollectObjDeclInfo::default();
            collector.run_visitor(&block);
            self.fns_without_this = collector.fns_without_this;
            self.known_strings = collector.known_strings;
            self.known_objs = collector.known_objs;
        }

        enter(self, block)
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        self.last_use_was_safe = false;

        match stmt {
            ir::Stmt::Expr { target: _, expr } => match expr {
                ir::Expr::ReadMember { obj, prop } => {
                    self.last_use_was_safe = true;
                    self.access_prop(Access::Read, obj, prop);
                    self.known_objs.remove(prop);
                }
                ir::Expr::Call {
                    kind: _,
                    base,
                    prop: Some(prop),
                    args,
                } => {
                    self.last_use_was_safe = true;
                    self.access_prop(Access::Call, base, prop);
                    self.known_objs.remove(prop);
                    for (_, arg) in args {
                        self.known_objs.remove(arg);
                    }
                }
                _ => {}
            },
            ir::Stmt::WriteMember { obj, prop, val } => {
                self.last_use_was_safe = true;
                self.write_prop(obj, prop, val);
                self.known_objs.remove(prop);
                self.known_objs.remove(val);
            }
            _ => {}
        }
    }

    fn visit_ref_use(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
        if !self.last_use_was_safe {
            self.known_objs.remove(ref_);
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
            let mut collector = CollectObjAccessInfo::default();
            collector.run_visitor(&block);
            self.objects_to_replace = collector
                .known_objs
                .into_iter()
                .filter_map(|(obj, props)| {
                    let prop_vars = props
                        .into_iter()
                        .map(|(prop, info)| {
                            if info.safe_for_call || !info.saw_call {
                                Ok((prop.to_string(), ir::Ref::new(prop)))
                            } else {
                                Err(())
                            }
                        })
                        .collect::<Result<_, _>>();
                    match prop_vars {
                        Ok(prop_vars) => Some((obj.weak(), prop_vars)),
                        Err(()) => None,
                    }
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
            ir::Stmt::Expr {
                target,
                expr:
                    ir::Expr::Call {
                        kind,
                        base,
                        prop: Some(prop),
                        args,
                    },
            } => match self.objects_to_replace.get(&base.weak()) {
                Some(prop_vars) => {
                    let prop_var = match self
                        .known_strings
                        .get(&prop.weak())
                        .and_then(|name| prop_vars.get(name))
                    {
                        Some(prop_var) => prop_var.clone(),
                        None => unreachable!("unknown prop"),
                    };
                    let prop_ref = ir::Ref::new(prop_var.name_hint());
                    Many(vec![
                        ir::Stmt::Expr {
                            target: prop_ref.clone(),
                            expr: ir::Expr::ReadMutable { source: prop_var },
                        },
                        ir::Stmt::Expr {
                            target,
                            expr: ir::Expr::Call {
                                kind,
                                base: prop_ref,
                                prop: None,
                                args,
                            },
                        },
                    ])
                }
                None => One(ir::Stmt::Expr {
                    target,
                    expr: ir::Expr::Call {
                        kind,
                        base,
                        prop: Some(prop),
                        args,
                    },
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
