use crate::emit;
use crate::err::NiceError;
use crate::opt_ast;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let ast = opt_ast::run(g, ast);
                let js = emit::emit(g, ast, files, emit::Opt { minify: false })?;
                insta::assert_snapshot!(js, @ $expected);
                Ok(())
            })
        }
    };
}

case!(basic_empty_if, r#"
    if (x) {
        console.log(1);
    } else {
        console.log(2);
        console.log(3);
    }
"#, @r###"
if (x) console.log(1);
else {
    console.log(2);
    console.log(3);
}
"###);

case!(chained_single_stmt_ifs, r#"
    if (x) {
        if (y) {
            console.log(1);
        } else if (z) {
            console.log(2);
        }
    // this else should not get attached to the inner if-elseif
    } else {
        console.log(3);
    }
"#, @r###"
if (x) {
    if (y) console.log(1);
    else if (z) console.log(2);
} else console.log(3);
"###);

case!(if_zero_bitor, r#"
    if (0 | x) {
        first();
    } else {
        second();
    }
"#, @r###"
if (0 | x) first();
else second();
"###);
