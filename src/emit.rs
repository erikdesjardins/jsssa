use std::sync::Arc;

use failure::Error;
use swc_common::{FilePathMapping, SourceMap};
use swc_ecma_ast as ast;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter, Handlers};

use crate::swc_globals;

pub fn emit(_: &swc_globals::Initialized, ast: ast::Script) -> Result<String, Error> {
    let mut wr = vec![];

    {
        let files = Arc::new(SourceMap::new(FilePathMapping::empty()));
        let mut emitter = Emitter {
            cfg: Default::default(),
            cm: files.clone(),
            wr: Box::new(JsWriter::new(files.clone(), "\n", &mut wr, None)),
            comments: None,
            handlers: {
                struct MyHandlers;
                impl Handlers for MyHandlers {}
                Box::new(MyHandlers)
            },
            pos_of_leading_comments: Default::default(),
        };
        emitter.emit_script(&ast)?;
    }

    Ok(String::from_utf8_lossy(&wr).into_owned())
}
