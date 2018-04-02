use std::ptr;

use failure::{Error, ResultExt};
use mozjs::conversions::{FromJSValConvertible, ToJSValConvertible};
use mozjs::jsapi::{CompartmentOptions, HandleValueArray, JSAutoCompartment, JS_NewGlobalObject,
                   OnNewGlobalHookOption};
use mozjs::jsval::UndefinedValue;
use mozjs::rust::wrappers::{JS_CallFunctionValue, JS_GetPendingException, JS_ErrorFromException};
use mozjs::rust::{Runtime, SIMPLE_GLOBAL_CLASS};
use serde_json;

use ast;
use ffi;

static BABYLON_BIN: &'static str = include_str!("../vendor/babylon.js");

// Embed mozjs (SpiderMonkey) to run Babylon for JS parsing.
// Yes, this is horrifying, but I don't want to port a JS parser to Rust right now.
pub fn parse(js: &str) -> Result<ast::File, Error> {
    let converted_ast;
    let babylon_ast = unsafe {
        let runtime = Runtime::new().expect("SpiderMonkey runtime failed to initialize");
        let context = runtime.cx();

        // prepare simple global object
        rooted!(in(context) let global = JS_NewGlobalObject(
            context,
            &SIMPLE_GLOBAL_CLASS,
            ptr::null_mut(),
            OnNewGlobalHookOption::FireOnNewGlobalHook,
            &CompartmentOptions::default()
        ));

        // RAII compartment (same as V8 isolate?)
        let _ac = JSAutoCompartment::new(context, global.get());

        // run global Babylon code, which returns our parse function
        rooted!(in(context) let mut parse_to_string_fn = UndefinedValue());
        assert!(runtime.evaluate_script(
            global.handle(),
            BABYLON_BIN,
            "<synthetic_jsssa_setup>",
            0,
            parse_to_string_fn.handle_mut()
        ).is_ok());
        assert!(parse_to_string_fn.is_object());

        // call our parse function
        rooted!(in(context) let mut js_js = UndefinedValue());
        js.to_jsval(context, js_js.handle_mut());
        assert!(js_js.is_string());
        rooted!(in(context) let mut parse_result = UndefinedValue());
        let succeeded = JS_CallFunctionValue(
            context,
            global.handle(),
            parse_to_string_fn.handle(),
            &HandleValueArray::from_rooted_slice(&[*js_js]) as *const HandleValueArray,
            parse_result.handle_mut()
        );

        if !succeeded {
            // read pending exception
            rooted!(in(context) let mut exception = UndefinedValue());
            assert!(JS_GetPendingException(context, exception.handle_mut()));
            rooted!(in(context) let exception_object = exception.to_object());
            let error = JS_ErrorFromException(context, exception_object.handle());
            assert_ne!(error, ptr::null_mut());
            let msg_ptr = (*error).ucmessage;
            assert_ne!(msg_ptr, ptr::null());
            let message = ffi::ucs2_to_string(msg_ptr);
            return Err(ParseError { message }.into());
        }

        assert!(parse_result.is_string());
        converted_ast = String::from_jsval(context, parse_result.handle(), ()).unwrap();
        converted_ast.get_success_value().expect("converting parse result to Rust string")
    };

    let ast = serde_json::from_str(babylon_ast).context("failed to parse babylon output")?;
    Ok(ast)
}

#[derive(Fail, Debug, PartialEq)]
#[fail(display = "JS parse error: {}", message)]
struct ParseError {
    message: String
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
            ).unwrap(),
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

    #[test]
    fn basic_error() {
        assert_eq!(
            parse("1..").unwrap_err().downcast::<ParseError>().unwrap(),
            ParseError { message: "Unexpected token (1:3)".to_string() }
        );
    }
}
