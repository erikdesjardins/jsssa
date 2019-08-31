use std::fmt::{self, Debug};

use crate::ir;

/// Non-mutating traversal, receiving each node by reference.
pub trait Visitor<'a>: Sized + Debug {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &'a ir::Block,
        enter: impl FnOnce(&mut Self, &'a ir::Block) -> R,
    ) -> R {
        let _ = ty;
        enter(self, block)
    }

    fn visit(&mut self, stmt: &'a ir::Stmt) {
        let _ = stmt;
    }

    fn visit_ref_use(&mut self, ref_: &'a ir::Ref<ir::Ssa>) {
        let _ = ref_;
    }
}

/// Mutating/mapping traversal, receiving each node by value.
pub trait Folder: Sized + Debug {
    type Output: IntoIterator<Item = ir::Stmt>;

    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: ir::Block,
        enter: impl FnOnce(&mut Self, ir::Block) -> R,
    ) -> R {
        let _ = ty;
        enter(self, block)
    }

    fn fold(&mut self, stmt: ir::Stmt) -> Self::Output;

    fn fold_ref_use(&mut self, ref_: ir::Ref<ir::Ssa>) -> ir::Ref<ir::Ssa> {
        ref_
    }
}

/// Execute a visitor on borrowed IR.
pub trait RunVisitor<'a>: Visitor<'a> {
    fn run_visitor(&mut self, ir: &'a ir::Block) {
        visit_block(self, ir, ScopeTy::Toplevel)
    }
}

impl<'a, V: Visitor<'a>> RunVisitor<'a> for V {}

/// Execute a folder on owned IR.
pub trait RunFolder: Folder {
    fn run_folder(&mut self, ir: ir::Block) -> ir::Block {
        fold_block(self, ir, ScopeTy::Toplevel)
    }
}

impl<F: Folder> RunFolder for F {}

/// Helper to run simple visitors without defining a struct.
pub fn visit_with<'a>(ir: &'a ir::Block, f: impl FnMut(&'a ir::Stmt)) {
    struct VisitFn<F>(F);
    impl<F> Debug for VisitFn<F> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("<visitor fn>")
        }
    }
    impl<'a, F: FnMut(&'a ir::Stmt)> Visitor<'a> for VisitFn<F> {
        fn visit(&mut self, stmt: &'a ir::Stmt) {
            (self.0)(stmt);
        }
    }
    VisitFn(f).run_visitor(ir);
}

/// Type of scope about to be entered.
pub enum ScopeTy {
    Normal,
    Toplevel,
    Nonlinear,
    Function,
}

fn visit_block<'a>(this: &mut impl Visitor<'a>, block: &'a ir::Block, ty: ScopeTy) {
    this.wrap_scope(&ty, block, |this, block| {
        let ir::Block(children) = block;

        children.iter().for_each(|child| {
            this.visit(child);

            match child {
                ir::Stmt::Expr { target: _, expr } => match expr {
                    ir::Expr::Read { source } => {
                        this.visit_ref_use(source);
                    }
                    ir::Expr::ReadMember { obj, prop } => {
                        this.visit_ref_use(obj);
                        this.visit_ref_use(prop);
                    }
                    ir::Expr::Array { elems } => {
                        elems.iter().flatten().for_each(|(_kind, ele)| {
                            this.visit_ref_use(ele);
                        });
                    }
                    ir::Expr::Object { props } => {
                        props.iter().for_each(|(_kind, key, val)| {
                            this.visit_ref_use(key);
                            this.visit_ref_use(val);
                        });
                    }
                    ir::Expr::Unary { op: _, val } => {
                        this.visit_ref_use(val);
                    }
                    ir::Expr::Binary { op: _, left, right } => {
                        this.visit_ref_use(left);
                        this.visit_ref_use(right);
                    }
                    ir::Expr::Delete { obj, prop } => {
                        this.visit_ref_use(obj);
                        this.visit_ref_use(prop);
                    }
                    ir::Expr::Yield { kind: _, val } => {
                        this.visit_ref_use(val);
                    }
                    ir::Expr::Await { val } => {
                        this.visit_ref_use(val);
                    }
                    ir::Expr::Call {
                        kind: _,
                        base,
                        prop,
                        args,
                    } => {
                        this.visit_ref_use(base);
                        prop.as_ref().map(|prop| this.visit_ref_use(prop));
                        args.iter().for_each(|(_kind, arg)| {
                            this.visit_ref_use(arg);
                        });
                    }
                    ir::Expr::Function { kind: _, body } => {
                        visit_block(this, body, ScopeTy::Function);
                    }
                    ir::Expr::Bool { value: _ }
                    | ir::Expr::Number { value: _ }
                    | ir::Expr::String { value: _ }
                    | ir::Expr::Null
                    | ir::Expr::Undefined
                    | ir::Expr::This
                    | ir::Expr::ReadMutable { source: _ }
                    | ir::Expr::ReadGlobal { source: _ }
                    | ir::Expr::RegExp { regex: _, flags: _ }
                    | ir::Expr::CurrentFunction
                    | ir::Expr::Argument { index: _ } => {}
                },
                ir::Stmt::DeclareMutable { target: _, val } => {
                    this.visit_ref_use(val);
                }
                ir::Stmt::WriteMutable { target: _, val } => {
                    this.visit_ref_use(val);
                }
                ir::Stmt::WriteGlobal { target: _, val } => {
                    this.visit_ref_use(val);
                }
                ir::Stmt::WriteMember { obj, prop, val } => {
                    this.visit_ref_use(obj);
                    this.visit_ref_use(prop);
                    this.visit_ref_use(val);
                }
                ir::Stmt::Return { val } => {
                    this.visit_ref_use(val);
                }
                ir::Stmt::Throw { val } => {
                    this.visit_ref_use(val);
                }
                ir::Stmt::Label { label: _, body } => {
                    visit_block(this, body, ScopeTy::Normal);
                }
                ir::Stmt::Loop { body } => {
                    visit_block(this, body, ScopeTy::Nonlinear);
                }
                ir::Stmt::ForEach {
                    kind: _,
                    init,
                    body,
                } => {
                    this.visit_ref_use(init);
                    visit_block(this, body, ScopeTy::Nonlinear);
                }
                ir::Stmt::IfElse { cond, cons, alt } => {
                    this.visit_ref_use(cond);
                    visit_block(this, cons, ScopeTy::Normal);
                    visit_block(this, alt, ScopeTy::Normal);
                }
                ir::Stmt::Switch { discr, body } => {
                    this.visit_ref_use(discr);
                    visit_block(this, body, ScopeTy::Normal);
                }
                ir::Stmt::SwitchCase { val } => {
                    val.as_ref().map(|val| this.visit_ref_use(val));
                }
                ir::Stmt::Try {
                    body,
                    catch,
                    finally,
                } => {
                    visit_block(this, body, ScopeTy::Normal);
                    visit_block(this, catch, ScopeTy::Normal);
                    visit_block(this, finally, ScopeTy::Normal);
                }
                ir::Stmt::Break { label: _ }
                | ir::Stmt::Continue { label: _ }
                | ir::Stmt::Debugger => {}
            }
        });
    })
}

fn fold_block(this: &mut impl Folder, block: ir::Block, ty: ScopeTy) -> ir::Block {
    this.wrap_scope(&ty, block, |this, block| {
        let ir::Block(children) = block;

        let folded_children = children
            .into_iter()
            .flat_map(|child| {
                this.fold(child)
                    .into_iter()
                    .map(|stmt| match stmt {
                        ir::Stmt::Expr { target, expr } => ir::Stmt::Expr {
                            target,
                            expr: match expr {
                                ir::Expr::Read { source } => ir::Expr::Read {
                                    source: this.fold_ref_use(source),
                                },
                                ir::Expr::ReadMember { obj, prop } => ir::Expr::ReadMember {
                                    obj: this.fold_ref_use(obj),
                                    prop: this.fold_ref_use(prop),
                                },
                                ir::Expr::Array { elems } => ir::Expr::Array {
                                    elems: elems
                                        .into_iter()
                                        .map(|opt| {
                                            opt.map(|(kind, ele)| (kind, this.fold_ref_use(ele)))
                                        })
                                        .collect(),
                                },
                                ir::Expr::Object { props } => ir::Expr::Object {
                                    props: props
                                        .into_iter()
                                        .map(|(kind, key, val)| {
                                            (kind, this.fold_ref_use(key), this.fold_ref_use(val))
                                        })
                                        .collect(),
                                },
                                ir::Expr::Unary { op, val } => ir::Expr::Unary {
                                    op,
                                    val: this.fold_ref_use(val),
                                },
                                ir::Expr::Binary { op, left, right } => ir::Expr::Binary {
                                    op,
                                    left: this.fold_ref_use(left),
                                    right: this.fold_ref_use(right),
                                },
                                ir::Expr::Delete { obj, prop } => ir::Expr::Delete {
                                    obj: this.fold_ref_use(obj),
                                    prop: this.fold_ref_use(prop),
                                },
                                ir::Expr::Yield { kind, val } => ir::Expr::Yield {
                                    kind,
                                    val: this.fold_ref_use(val),
                                },
                                ir::Expr::Await { val } => ir::Expr::Await {
                                    val: this.fold_ref_use(val),
                                },
                                ir::Expr::Call {
                                    kind,
                                    base,
                                    prop,
                                    args,
                                } => ir::Expr::Call {
                                    kind,
                                    base: this.fold_ref_use(base),
                                    prop: prop.map(|prop| this.fold_ref_use(prop)),
                                    args: args
                                        .into_iter()
                                        .map(|(kind, arg)| (kind, this.fold_ref_use(arg)))
                                        .collect(),
                                },
                                ir::Expr::Function { kind, body } => ir::Expr::Function {
                                    kind,
                                    body: fold_block(this, body, ScopeTy::Function),
                                },
                                ir::Expr::Bool { value: _ }
                                | ir::Expr::Number { value: _ }
                                | ir::Expr::String { value: _ }
                                | ir::Expr::Null
                                | ir::Expr::Undefined
                                | ir::Expr::This
                                | ir::Expr::ReadMutable { source: _ }
                                | ir::Expr::ReadGlobal { source: _ }
                                | ir::Expr::RegExp { regex: _, flags: _ }
                                | ir::Expr::CurrentFunction
                                | ir::Expr::Argument { index: _ } => expr,
                            },
                        },
                        ir::Stmt::DeclareMutable { target, val } => ir::Stmt::DeclareMutable {
                            target,
                            val: this.fold_ref_use(val),
                        },
                        ir::Stmt::WriteMutable { target, val } => ir::Stmt::WriteMutable {
                            target,
                            val: this.fold_ref_use(val),
                        },
                        ir::Stmt::WriteGlobal { target, val } => ir::Stmt::WriteGlobal {
                            target,
                            val: this.fold_ref_use(val),
                        },
                        ir::Stmt::WriteMember { obj, prop, val } => ir::Stmt::WriteMember {
                            obj: this.fold_ref_use(obj),
                            prop: this.fold_ref_use(prop),
                            val: this.fold_ref_use(val),
                        },
                        ir::Stmt::Return { val } => ir::Stmt::Return {
                            val: this.fold_ref_use(val),
                        },
                        ir::Stmt::Throw { val } => ir::Stmt::Throw {
                            val: this.fold_ref_use(val),
                        },
                        ir::Stmt::Label { label, body } => ir::Stmt::Label {
                            label,
                            body: fold_block(this, body, ScopeTy::Normal),
                        },
                        ir::Stmt::Loop { body } => ir::Stmt::Loop {
                            body: fold_block(this, body, ScopeTy::Nonlinear),
                        },
                        ir::Stmt::ForEach { kind, init, body } => ir::Stmt::ForEach {
                            kind,
                            init: this.fold_ref_use(init),
                            body: fold_block(this, body, ScopeTy::Nonlinear),
                        },
                        ir::Stmt::IfElse { cond, cons, alt } => ir::Stmt::IfElse {
                            cond: this.fold_ref_use(cond),
                            cons: fold_block(this, cons, ScopeTy::Normal),
                            alt: fold_block(this, alt, ScopeTy::Normal),
                        },
                        ir::Stmt::Switch { discr, body } => ir::Stmt::Switch {
                            discr: this.fold_ref_use(discr),
                            body: fold_block(this, body, ScopeTy::Normal),
                        },
                        ir::Stmt::SwitchCase { val } => ir::Stmt::SwitchCase {
                            val: val.map(|val| this.fold_ref_use(val)),
                        },
                        ir::Stmt::Try {
                            body,
                            catch,
                            finally,
                        } => ir::Stmt::Try {
                            body: fold_block(this, body, ScopeTy::Normal),
                            catch: fold_block(this, catch, ScopeTy::Normal),
                            finally: Box::new(fold_block(this, *finally, ScopeTy::Normal)),
                        },
                        ir::Stmt::Break { label: _ }
                        | ir::Stmt::Continue { label: _ }
                        | ir::Stmt::Debugger => stmt,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        ir::Block(folded_children)
    })
}
