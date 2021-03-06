use std::fmt::{self, Display};
use std::io;
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use swc_common::errors::emitter::EmitterWriter;
use swc_common::errors::Handler;
use swc_common::{FileName, FilePathMapping, SourceMap};
use swc_ecma_ast as ast;
use swc_ecma_parser::{Parser, StringInput, Syntax};

use crate::swc_globals;

/// Parse a given ES6+ script into SWC's AST.
#[inline(never)] // for better profiling
pub fn parse(
    _: &swc_globals::Initialized,
    js: impl Into<String>,
) -> Result<(ast::Program, Rc<SourceMap>), ParseError> {
    let files = Rc::new(SourceMap::new(FilePathMapping::empty()));

    let error = BufferedError::default();
    let emitter = EmitterWriter::new(Box::new(error.clone()), Some(files.clone()), false, false);
    let handler = Handler::with_emitter_and_flags(Box::new(emitter), Default::default());

    let file = files.new_source_file(FileName::Anon, js.into());

    let mut parser = Parser::new(
        Syntax::Es(Default::default()),
        StringInput::from(file.as_ref()),
        None,
    );

    let ast = match parser.parse_script() {
        Ok(script) => {
            // we may still receive an AST for partial parse results, so check for errors
            for e in parser.take_errors() {
                e.into_diagnostic(&handler).emit();
            }
            Some(ast::Program::Script(script))
        }
        Err(e) => {
            e.into_diagnostic(&handler).emit();
            None
        }
    };

    let err = error.read_msg();

    match (ast, err) {
        (_, Some(err)) => Err(ParseError(err)),
        (Some(ast), None) => Ok((ast, files)),
        (None, None) => unreachable!("parse failed, but no error message was emitted"),
    }
}

#[derive(Debug)]
pub struct ParseError(String);

impl std::error::Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parse {}", self.0)
    }
}

#[derive(Clone, Default)]
struct BufferedError(Arc<Mutex<Option<Vec<u8>>>>);

impl Write for BufferedError {
    fn write(&mut self, d: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().get_or_insert_with(Vec::new).write(d)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl BufferedError {
    fn read_msg(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .map(|v| String::from_utf8_lossy(v).into_owned())
    }
}
