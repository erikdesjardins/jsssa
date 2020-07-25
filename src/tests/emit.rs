use crate::emit;
use crate::err::NiceError;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr, @ $expected:literal ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, files) = parse::parse(g, $string)?;
                let js = emit::emit(g, ast, files, emit::Opt { minify: false })?;
                insta::assert_snapshot!(js, @ $expected);
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
    f(2), true;
"#,
    @r###"
function f(x) {
    while(true);
    x = y.bar;
    z.foo = x ? true : 'hi';
    return +[
        1 || x,
        {
            x
        },
        f + 1,
        ++g
    ];
}
f(2), true;
"###
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
        let js = emit::emit(g, ast, files, emit::Opt { minify: false })?;
        assert_eq!(
            js,
            r#"'\x001';
'\x008';
"#
        );
        Ok(())
    })
}

#[test]
fn minify() -> Result<(), NiceError> {
    swc_globals::with(|g| {
        let (ast, files) = parse::parse(
            g,
            r#"
            if (x) {
                y;
            }
        "#,
        )?;
        let js = emit::emit(g, ast, files, emit::Opt { minify: true })?;
        insta::assert_snapshot!(js, @"if(x){y;}");
        Ok(())
    })
}
