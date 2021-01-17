macro_rules! case {
    ( $name:ident, || $fold_expr:expr, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), crate::err::NiceError> {
            use swc_ecma_visit::FoldWith;
            use crate::{emit, parse, swc_globals};
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let ast = ast.fold_with(&mut $fold_expr);
                let js = emit::emit(g, ast, files, emit::Opt { minify: false })?;
                insta::assert_snapshot!(js, @ $expected);
                Ok(())
            })
        }
    };
    ( $name:ident, all_passes, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), crate::err::NiceError> {
            use crate::{emit, opt_ast, parse, swc_globals};
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let ast = opt_ast::run(g, ast, opt_ast::Opt { minify: false });
                let js = emit::emit(g, ast, files, emit::Opt { minify: false })?;
                insta::assert_snapshot!(js, @ $expected);
                Ok(())
            })
        }
    };
}

mod if2cond;
mod merge_vars;
mod resugar_loops;
mod swc;
