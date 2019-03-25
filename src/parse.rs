use std::sync::Arc;

use failure::Error;
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, FilePathMapping, SourceMap,
};
use swc_ecma_ast as ast;
use swc_ecma_parser::{Parser, Session, SourceFileInput, Syntax};

use crate::swc_globals;

/// Parse a given ES6+ script into SWC's AST.
pub fn parse(_: &swc_globals::Initialized, js: impl Into<String>) -> Result<ast::Script, Error> {
    let files = Arc::new(SourceMap::new(FilePathMapping::empty()));

    let session = Session {
        handler: &{
            let warnings = false;
            Handler::with_tty_emitter(ColorConfig::Auto, warnings, false, Some(files.clone()))
        },
    };

    let file = files.new_source_file(FileName::Custom("jsssa_filename.js".to_string()), js.into());

    let mut parser = Parser::new(
        session,
        Syntax::Es(Default::default()),
        SourceFileInput::from(file.as_ref()),
        None,
    );

    let ast = parser.parse_script().map_err(|mut e| {
        e.emit();
        unimplemented!("proper error reporting");
    });

    ast
}
