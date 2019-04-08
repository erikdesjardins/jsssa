use std::sync::Arc;

use swc_common::SourceMap;
use swc_ecma_ast as ast;
use swc_ecma_codegen::{text_writer::JsWriter, Config, Emitter, Handlers};

use crate::err::Error;
use crate::swc_globals;

pub struct Opt {
    pub minify: bool,
}

#[inline(never)] // for better profiling
pub fn emit(
    _: &swc_globals::Initialized,
    ast: ast::Script,
    files: Arc<SourceMap>,
    options: Opt,
) -> Result<String, Error> {
    let mut wr = vec![];

    {
        let mut emitter = Emitter {
            cfg: Config {
                minify: options.minify,
            },
            cm: files.clone(),
            wr: Box::new(JsWriter::new(files, "\n", &mut wr, None)),
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
