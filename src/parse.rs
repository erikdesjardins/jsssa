use std::env;
use std::fs::File;
use std::process::{Command, Stdio};
use std::io::Write;

use rand::{self, Rng};
use serde_json;

use ast;

static BABYLON_BIN: &[u8] = include_bytes!("../vendor/babylon.js");

pub fn parse(js: &str) -> ast::File {
    let mut path = env::temp_dir();
    let random_name: String = rand::thread_rng().gen_ascii_chars().take(32).collect();
    path.push(random_name);

    File::create(&path).unwrap().write_all(BABYLON_BIN).unwrap();

    let mut child = Command::new("node")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    write!(child.stdin.as_mut().unwrap(), "{}", js).unwrap();

    let out = child.wait_with_output().unwrap().stdout;

    let str = String::from_utf8(out).unwrap();

    serde_json::from_str(&str).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_foo() {
        use ast::*;

        assert_eq!(
            parse(
                r#"
                    'use strict';
                    function foo(x) {
                        return x + 1;
                    }
                "#
            ),
            File::new(Program::new(
                vec![
                    FunctionDeclaration::new(
                        Identifier::new("foo".to_owned()),
                        vec![Identifier::new("x".to_owned()).into()],
                        BlockStatement::new(
                            vec![
                                ReturnStatement::new(Some(
                                    BinaryExpression::new(
                                        BinaryOperator::Add,
                                        Box::new(Identifier::new("x".to_owned()).into()),
                                        Box::new(NumericLiteral::new(1.0).into()),
                                    ).into(),
                                )).into(),
                            ],
                            vec![],
                        ),
                        false,
                        false,
                    ).into(),
                ],
                vec![
                    Directive::new(DirectiveLiteral::new("use strict".to_owned())),
                ],
                SourceType::Script,
            ))
        )
    }
}
