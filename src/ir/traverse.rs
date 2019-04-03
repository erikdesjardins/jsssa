use crate::ir;

/// Non-mutating traversal, receiving each node by reference.
pub trait Visitor: Sized {
    fn wrap_scope<R>(
        &mut self,
        ty: &ScopeTy,
        block: &ir::Block,
        enter: impl FnOnce(&mut Self, &ir::Block) -> R,
    ) -> R {
        let _ = ty;
        enter(self, block)
    }

    fn visit(&mut self, stmt: &ir::Stmt);
}

/// Mutating/mapping traversal, receiving each node by value.
pub trait Folder: Sized {
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
}

/// Execute a visitor on borrowed IR.
pub trait RunVisitor: Visitor {
    fn run_visitor(&mut self, ir: &ir::Block) {
        visit_block(self, ir, ScopeTy::Toplevel)
    }
}

impl<V: Visitor> RunVisitor for V {}

/// Execute a folder on owned IR.
pub trait RunFolder: Folder {
    fn run_folder(&mut self, ir: ir::Block) -> ir::Block {
        fold_block(self, ir, ScopeTy::Toplevel)
    }
}

impl<F: Folder> RunFolder for F {}

/// Helper to run simple visitors without defining a struct.
pub fn visit_with(ir: &ir::Block, f: impl FnMut(&ir::Stmt)) {
    struct VisitFn<F>(F);
    impl<F: FnMut(&ir::Stmt)> Visitor for VisitFn<F> {
        fn visit(&mut self, stmt: &ir::Stmt) {
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

fn visit_block(this: &mut impl Visitor, block: &ir::Block, ty: ScopeTy) {
    this.wrap_scope(&ty, block, |this, block| {
        let ir::Block(children) = block;

        children.iter().for_each(|child| {
            this.visit(child);

            match child {
                ir::Stmt::Expr { target: _, expr } => match expr {
                    ir::Expr::Function { kind: _, body } => {
                        visit_block(this, body, ScopeTy::Function);
                    }
                    ir::Expr::Bool { .. }
                    | ir::Expr::Number { .. }
                    | ir::Expr::String { .. }
                    | ir::Expr::Null
                    | ir::Expr::Undefined
                    | ir::Expr::This
                    | ir::Expr::Read { .. }
                    | ir::Expr::ReadMutable { .. }
                    | ir::Expr::ReadGlobal { .. }
                    | ir::Expr::ReadMember { .. }
                    | ir::Expr::Array { .. }
                    | ir::Expr::Object { .. }
                    | ir::Expr::RegExp { .. }
                    | ir::Expr::Unary { .. }
                    | ir::Expr::Binary { .. }
                    | ir::Expr::Delete { .. }
                    | ir::Expr::Yield { .. }
                    | ir::Expr::Await { .. }
                    | ir::Expr::Call { .. }
                    | ir::Expr::CurrentFunction
                    | ir::Expr::Argument { .. } => {}
                },
                ir::Stmt::Label { label: _, body } => {
                    visit_block(this, body, ScopeTy::Normal);
                }
                ir::Stmt::Loop { body } => {
                    visit_block(this, body, ScopeTy::Nonlinear);
                }
                ir::Stmt::ForEach {
                    kind: _,
                    init: _,
                    body,
                } => {
                    visit_block(this, body, ScopeTy::Nonlinear);
                }
                ir::Stmt::IfElse { cond: _, cons, alt } => {
                    visit_block(this, cons, ScopeTy::Normal);
                    visit_block(this, alt, ScopeTy::Normal);
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
                ir::Stmt::DeclareMutable { .. }
                | ir::Stmt::WriteMutable { .. }
                | ir::Stmt::WriteGlobal { .. }
                | ir::Stmt::WriteMember { .. }
                | ir::Stmt::Return { .. }
                | ir::Stmt::Throw { .. }
                | ir::Stmt::Break { .. }
                | ir::Stmt::Continue { .. }
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
                                ir::Expr::Function { kind, body } => ir::Expr::Function {
                                    kind,
                                    body: fold_block(this, body, ScopeTy::Function),
                                },
                                ir::Expr::Bool { .. }
                                | ir::Expr::Number { .. }
                                | ir::Expr::String { .. }
                                | ir::Expr::Null
                                | ir::Expr::Undefined
                                | ir::Expr::This
                                | ir::Expr::Read { .. }
                                | ir::Expr::ReadMutable { .. }
                                | ir::Expr::ReadGlobal { .. }
                                | ir::Expr::ReadMember { .. }
                                | ir::Expr::Array { .. }
                                | ir::Expr::Object { .. }
                                | ir::Expr::RegExp { .. }
                                | ir::Expr::Unary { .. }
                                | ir::Expr::Binary { .. }
                                | ir::Expr::Delete { .. }
                                | ir::Expr::Yield { .. }
                                | ir::Expr::Await { .. }
                                | ir::Expr::Call { .. }
                                | ir::Expr::CurrentFunction
                                | ir::Expr::Argument { .. } => expr,
                            },
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
                            init,
                            body: fold_block(this, body, ScopeTy::Nonlinear),
                        },
                        ir::Stmt::IfElse { cond, cons, alt } => ir::Stmt::IfElse {
                            cond,
                            cons: fold_block(this, cons, ScopeTy::Normal),
                            alt: fold_block(this, alt, ScopeTy::Normal),
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
                        ir::Stmt::DeclareMutable { .. }
                        | ir::Stmt::WriteMutable { .. }
                        | ir::Stmt::WriteGlobal { .. }
                        | ir::Stmt::WriteMember { .. }
                        | ir::Stmt::Return { .. }
                        | ir::Stmt::Throw { .. }
                        | ir::Stmt::Break { .. }
                        | ir::Stmt::Continue { .. }
                        | ir::Stmt::Debugger => stmt,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        ir::Block(folded_children)
    })
}
