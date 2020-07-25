use std::sync::Arc;

use swc_common::SourceMap;
use swc_ecma_ast as ast;
use swc_ecma_codegen::{text_writer::JsWriter, Config, Emitter, Handlers};
use swc_ecma_transforms as transforms;
use swc_ecma_visit::FoldWith;

use crate::err::Error;
use crate::swc_globals;

pub struct Opt {
    pub minify: bool,
}

#[inline(never)] // for better profiling
pub fn emit(
    _: &swc_globals::Initialized,
    ast: ast::Program,
    files: Arc<SourceMap>,
    options: Opt,
) -> Result<String, Error> {
    let mut wr = vec![];

    let fixed_ast = ast.fold_with(&mut transforms::fixer());

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
        };
        emitter.emit_program(&fixed_ast)?;
    }

    Ok(String::from_utf8_lossy(&wr).into_owned())
}
