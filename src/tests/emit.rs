use crate::emit;
use crate::err::NiceError;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let js = emit::emit(g, ast, files)?;
                insta::assert_snapshot_matches!(stringify!($name), js, $string);
                Ok(())
            })
        }
    };
}

case!(
    basic,
    r#"
    function f(x) {
        while (true);
        x = y.bar;
        z.foo = x ? true : 'hi';
        return +[1 || x, { x }, f + 1, ++g];
    }
    f(1), true;
"#
);

#[test]
fn no_octal_escapes() -> Result<(), NiceError> {
    swc_globals::with(|g| {
        let (ast, files) = parse::parse(
            g,
            r#"
            "\x001"; // === "\0" + "1"
            "\x008"; // === "\0" + "8"
        "#,
        )?;
        let js = emit::emit(g, ast, files)?;
        // record current incorrect behaviour
        assert_eq!(
            js,
            r#"'\01';
'\08';
"#
        );
        Ok(())
    })
}
