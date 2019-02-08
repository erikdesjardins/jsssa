use failure::Error;
use swc_common::{
    errors::{ColorConfig, Handler},
    sync::Lrc,
    FileName, FilePathMapping, SourceMap,
};
use swc_ecma_ast as ast;
use swc_ecma_parser::{Parser, Session, SourceFileInput, Syntax};

pub fn parse(js: impl Into<String>) -> Result<Vec<ast::Stmt>, Error> {
    swc_common::GLOBALS.set(&swc_common::Globals::new(), || {
        let files = Lrc::new(SourceMap::new(FilePathMapping::empty()));

        let session = Session {
            handler: &{
                let warnings = false;
                Handler::with_tty_emitter(ColorConfig::Auto, warnings, false, Some(files.clone()))
            },
        };

        let file =
            files.new_source_file(FileName::Custom("jsssa_filename.js".to_string()), js.into());

        let mut parser = Parser::new(
            session,
            Syntax::Es(Default::default()),
            SourceFileInput::from(file.as_ref()),
        );

        parser.parse_script().map_err(|mut e| {
            e.emit();
            unimplemented!("proper error reporting");
        })
    })
}
