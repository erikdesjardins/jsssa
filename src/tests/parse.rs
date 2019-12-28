use crate::err::NiceError;
use crate::parse;
use crate::swc_globals;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), NiceError> {
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                insta::assert_debug_snapshot!(stringify!($name), ast);
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
fn parse_error() {
    // swc successfully parses `var ab` and returns that AST, along with emitting an error
    swc_globals::with(|g| match parse::parse(g, "var ab-cd = 1;").err() {
        Some(err) => insta::assert_display_snapshot!("parse error", err),
        None => panic!("parse unexpectedly succeeded"),
    });
}

#[test]
fn parse_error_immediate() {
    // swc can't parse anything, making this different from `parse_error`
    swc_globals::with(|g| match parse::parse(g, "*").err() {
        Some(err) => insta::assert_display_snapshot!("parse error immediate", err),
        None => panic!("parse unexpectedly succeeded"),
    });
}

case!(
    string_has_escape_behavior,
    r#"
    "foo";
    "ba\r";
    "ba\\z";
"#
);
