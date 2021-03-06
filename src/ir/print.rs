use crate::ir;
use crate::ir::scope;
use crate::swc_globals;

#[inline(never)] // for better profiling
pub fn print(_: &swc_globals::Initialized, ir: &ir::Block) -> String {
    let mut s = String::new();
    print_block(ir, &scope::Ir::no_globals(), &mut WriteIndent::new(&mut s));
    s
}

fn print_block<'a, 'b: 'a>(
    block: &ir::Block,
    parent_scope: &scope::Ir,
    w: &'a mut WriteIndent<'b>,
) {
    let mut scope = parent_scope.nested();

    let ir::Block(children) = block;

    for stmt in children {
        match stmt {
            ir::Stmt::Expr { target, expr: _ } if !target.used().is_never() => {
                scope.declare_ssa(target.clone());
            }
            ir::Stmt::DeclareMutable { target, val: _ } => {
                scope.declare_mutable(target.clone());
            }
            _ => {}
        }
    }

    if children.is_empty() {
        w.start_line();
        w.write_str("<empty>");
    } else {
        for stmt in children {
            w.start_line();
            print_stmt(stmt, &mut scope, w);
        }
    }
}

fn print_stmt<'a, 'b: 'a>(stmt: &ir::Stmt, scope: &mut scope::Ir, w: &'a mut WriteIndent<'b>) {
    match stmt {
        ir::Stmt::Expr { target, expr } => {
            if target.used().is_never() {
                w.write_str("<dead>");
            } else {
                match scope.get_ssa(target) {
                    Some(name) => w.write_str(&name),
                    None => w.write_str(&format!("<BAD {:?}>", target)),
                }
            }
            w.write_str(" = ");
            print_expr(expr, scope, w);
        }
        ir::Stmt::DeclareMutable { target, val } => {
            match scope.get_mutable(target) {
                Some(name) => w.write_str(&name),
                None => w.write_str(&format!("<BAD {:?}>", target)),
            }
            w.write_str(" <= ");
            print_ssa(val, scope, w);
        }
        ir::Stmt::WriteMutable { target, val } => match scope.get_mutable(target) {
            Some(name) => {
                w.write_str(&name);
                w.write_str(" <- ");
                print_ssa(val, scope, w);
            }
            None => w.write_str(&format!("<BAD {:?}>", target)),
        },
        ir::Stmt::WriteGlobal { target, val } => {
            w.write_str("<global ");
            w.write_str(target);
            w.write_str("> <- ");
            print_ssa(val, scope, w);
        }
        ir::Stmt::WriteMember { obj, prop, val } => {
            print_ssa(obj, scope, w);
            w.write_str("[");
            print_ssa(prop, scope, w);
            w.write_str("] <- ");
            print_ssa(val, scope, w);
        }
        ir::Stmt::Return { val } => {
            w.write_str("<return> ");
            print_ssa(val, scope, w);
        }
        ir::Stmt::Throw { val } => {
            w.write_str("<throw> ");
            print_ssa(val, scope, w);
        }
        ir::Stmt::Break { label } => {
            w.write_str("<break");
            if let Some(label) = label {
                match scope.get_label(label) {
                    Some(name) => {
                        w.write_str(" ");
                        w.write_str(&name);
                    }
                    None => w.write_str(&format!(" BAD {:?}", label)),
                }
            }
            w.write_str(">");
        }
        ir::Stmt::Continue { label } => {
            w.write_str("<continue");
            if let Some(label) = label {
                match scope.get_label(label) {
                    Some(name) => {
                        w.write_str(" ");
                        w.write_str(&name);
                    }
                    None => w.write_str(&format!(" BAD {:?}", label)),
                }
            }
            w.write_str(">");
        }
        ir::Stmt::Debugger => {
            w.write_str("<debugger>");
        }
        ir::Stmt::Label { label, body } => {
            let mut label_scope = scope.nested();
            let name = label_scope.declare_label(label.clone());
            w.write_str("<label ");
            w.write_str(&name);
            w.write_str(">:");
            print_block(body, &label_scope, &mut w.indented());
        }
        ir::Stmt::Loop { body } => {
            w.write_str("<loop>:");
            print_block(body, scope, &mut w.indented());
        }
        ir::Stmt::ForEach { kind, init, body } => {
            w.write_str("<foreach");
            w.write_str(match kind {
                ir::ForKind::In => " in> ",
                ir::ForKind::Of => " of> ",
            });
            print_ssa(init, scope, w);
            w.write_str(":");
            print_block(body, scope, &mut w.indented());
        }
        ir::Stmt::IfElse { cond, cons, alt } => {
            w.write_str("<if> ");
            print_ssa(cond, scope, w);
            w.write_str(":");
            print_block(cons, scope, &mut w.indented());
            w.start_line();
            w.write_str("<else>:");
            print_block(alt, scope, &mut w.indented());
        }
        ir::Stmt::Switch { discr, body } => {
            w.write_str("<switch> ");
            print_ssa(discr, scope, w);
            w.write_str(":");
            print_block(body, scope, &mut w.indented());
        }
        ir::Stmt::SwitchCase { val } => match val {
            Some(val) => {
                w.write_str("<case> ");
                print_ssa(val, scope, w);
                w.write_str(":");
            }
            None => w.write_str("<default>:"),
        },
        ir::Stmt::Try {
            body,
            catch,
            finally,
        } => {
            w.write_str("<try>:");
            print_block(body, scope, &mut w.indented());
            w.start_line();
            w.write_str("<catch>:");
            print_block(catch, scope, &mut w.indented());
            w.start_line();
            w.write_str("<finally>:");
            print_block(finally, scope, &mut w.indented());
        }
    }
}

fn print_expr<'a, 'b: 'a>(expr: &ir::Expr, scope: &scope::Ir, w: &'a mut WriteIndent<'b>) {
    match expr {
        ir::Expr::Bool { value } => w.write_str(&value.to_string()),
        ir::Expr::Number { value } => {
            if value.is_nan() {
                w.write_str("+NaN");
            } else {
                w.write_str(&value.to_string());
            }
        }
        ir::Expr::String { value } => {
            // use debug to print newlines etc. escaped
            w.write_str(&format!("{:?}", value.as_ref()));
        }
        ir::Expr::Null => w.write_str("<null>"),
        ir::Expr::Undefined => w.write_str("<void>"),
        ir::Expr::This => w.write_str("<this>"),
        ir::Expr::Read { source } => print_ssa(source, scope, w),
        ir::Expr::ReadMutable { source } => {
            w.write_str("*");
            match scope.get_mutable(source) {
                Some(name) => w.write_str(&name),
                None => w.write_str(&format!("<BAD {:?}>", source)),
            }
        }
        ir::Expr::ReadGlobal { source } => {
            w.write_str("<global ");
            w.write_str(source);
            w.write_str(">");
        }
        ir::Expr::ReadMember { obj, prop } => {
            print_ssa(obj, scope, w);
            w.write_str("[");
            print_ssa(prop, scope, w);
            w.write_str("]");
        }
        ir::Expr::Array { elems } => {
            w.write_str("[");
            for (i, elem) in elems.iter().enumerate() {
                if let Some((kind, val)) = elem {
                    w.write_str(match kind {
                        ir::EleKind::Single => "",
                        ir::EleKind::Spread => "...",
                    });
                    print_ssa(val, scope, w);
                }
                if i < elems.len() - 1 {
                    w.write_str(", ");
                }
            }
            w.write_str("]");
        }
        ir::Expr::Object { props } => {
            w.write_str("{ ");
            for (i, (kind, prop, val)) in props.iter().enumerate() {
                w.write_str(match kind {
                    ir::PropKind::Simple => "",
                    ir::PropKind::Get => "<get> ",
                    ir::PropKind::Set => "<set> ",
                });
                w.write_str("[");
                print_ssa(prop, scope, w);
                w.write_str("]: ");
                print_ssa(val, scope, w);
                if i < props.len() - 1 {
                    w.write_str(", ");
                }
            }
            w.write_str(" }");
        }
        ir::Expr::RegExp { regex, flags } => {
            w.write_str("/");
            w.write_str(regex);
            w.write_str("/");
            w.write_str(flags);
        }
        ir::Expr::Unary { op, val } => {
            w.write_str(match op {
                ir::UnaryOp::Plus => "+ ",
                ir::UnaryOp::Minus => "- ",
                ir::UnaryOp::Not => "! ",
                ir::UnaryOp::Tilde => "! ",
                ir::UnaryOp::Typeof => "<typeof> ",
                ir::UnaryOp::Void => "<void> ",
            });
            print_ssa(val, scope, w);
        }
        ir::Expr::Binary { op, left, right } => {
            print_ssa(left, scope, w);
            w.write_str(match op {
                ir::BinaryOp::EqEq => " == ",
                ir::BinaryOp::NotEq => " != ",
                ir::BinaryOp::StrictEq => " === ",
                ir::BinaryOp::NotStrictEq => " !== ",
                ir::BinaryOp::Lt => " < ",
                ir::BinaryOp::LtEq => " <= ",
                ir::BinaryOp::Gt => " > ",
                ir::BinaryOp::GtEq => " >= ",
                ir::BinaryOp::ShiftLeft => " << ",
                ir::BinaryOp::ShiftRight => " >> ",
                ir::BinaryOp::ShiftRightZero => " >>> ",
                ir::BinaryOp::Add => " + ",
                ir::BinaryOp::Sub => " - ",
                ir::BinaryOp::Mul => " * ",
                ir::BinaryOp::Div => " / ",
                ir::BinaryOp::Mod => " % ",
                ir::BinaryOp::BitOr => " | ",
                ir::BinaryOp::BitXor => " ^ ",
                ir::BinaryOp::BitAnd => " & ",
                ir::BinaryOp::Exp => " ** ",
                ir::BinaryOp::In => " <in> ",
                ir::BinaryOp::Instanceof => " <instanceof> ",
            });
            print_ssa(right, scope, w);
        }
        ir::Expr::Delete { obj, prop } => {
            w.write_str("<delete> ");
            print_ssa(obj, scope, w);
            w.write_str("[");
            print_ssa(prop, scope, w);
            w.write_str("]");
        }
        ir::Expr::Yield { kind, val } => {
            w.write_str(match kind {
                ir::YieldKind::Single => "<yield> ",
                ir::YieldKind::Delegate => "<yield*> ",
            });
            print_ssa(val, scope, w);
        }
        ir::Expr::Await { val } => {
            w.write_str("<await> ");
            print_ssa(val, scope, w);
        }
        ir::Expr::Call {
            kind,
            base,
            prop,
            args,
        } => {
            w.write_str(match kind {
                ir::CallKind::Call => "",
                ir::CallKind::New => "<new> ",
            });
            print_ssa(base, scope, w);
            if let Some(prop) = prop {
                w.write_str("[");
                print_ssa(prop, scope, w);
                w.write_str("]");
            }
            w.write_str("(");
            for (i, (kind, val)) in args.iter().enumerate() {
                w.write_str(match kind {
                    ir::EleKind::Single => "",
                    ir::EleKind::Spread => "...",
                });
                print_ssa(val, scope, w);
                if i < args.len() - 1 {
                    w.write_str(", ");
                }
            }
            w.write_str(")");
        }
        ir::Expr::Function { kind, body } => {
            match kind {
                ir::FnKind::Arrow { is_async } => {
                    w.write_str("<arrow");
                    if *is_async {
                        w.write_str(" async");
                    }
                    w.write_str(">:");
                }
                ir::FnKind::Func {
                    is_async,
                    is_generator,
                } => {
                    w.write_str("<function");
                    if *is_async {
                        w.write_str(" async");
                    }
                    if *is_generator {
                        w.write_str(" generator");
                    }
                    w.write_str(">:");
                }
            }
            print_block(body, scope, &mut w.indented());
        }
        ir::Expr::CurrentFunction => {
            w.write_str("<current function>");
        }
        ir::Expr::Argument { index } => {
            w.write_str("<argument ");
            w.write_str(&index.to_string());
            w.write_str(">");
        }
    }
}

fn print_ssa(ssa_ref: &ir::Ref<ir::Ssa>, scope: &scope::Ir, w: &mut WriteIndent<'_>) {
    match scope.get_ssa(ssa_ref) {
        Some(name) => w.write_str(&name),
        None => w.write_str(&format!("<BAD {:?}>", ssa_ref)),
    }
}

struct WriteIndent<'a> {
    string: &'a mut String,
    spaces: u32,
}

impl<'b> WriteIndent<'b> {
    fn new(s: &'b mut String) -> WriteIndent<'b> {
        Self {
            string: s,
            spaces: 0,
        }
    }

    fn write_str(&mut self, s: &str) {
        self.string.push_str(s);
    }

    fn start_line(&mut self) {
        if !self.string.is_empty() {
            self.string.push('\n');
        }
        for _ in 0..self.spaces {
            self.string.push(' ');
        }
    }
}

trait Indented<'a> {
    fn indented(self) -> WriteIndent<'a>;
}

impl<'a, 'b: 'a> Indented<'a> for &'a mut WriteIndent<'b> {
    fn indented(self) -> WriteIndent<'a> {
        WriteIndent {
            string: self.string,
            spaces: self.spaces + 4,
        }
    }
}
