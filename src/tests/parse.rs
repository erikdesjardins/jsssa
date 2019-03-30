use crate::parse;
use crate::swc_globals;
use crate::utils::DisplayError;

macro_rules! case {
    ( $name:ident, $string:expr ) => {
        #[test]
        fn $name() -> Result<(), DisplayError> {
            swc_globals::with(|g| {
                let (ast, _) = parse::parse(g, $string)?;
                insta::assert_debug_snapshot_matches!(stringify!($name), ast);
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
    swc_globals::with(|g| {
        let err = parse::parse(g, "var ab-cd = 1;").err().unwrap();
        insta::assert_display_snapshot_matches!("parse error", err);
    });
}
