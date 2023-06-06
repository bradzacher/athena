use std::path::PathBuf;
use swc_common::{
    errors::{ColorConfig, Handler},
    sync::Lrc,
    SourceMap,
};
use swc_ecma_ast::{EsVersion, Program};
use swc_ecma_parser::{lexer::Lexer, Capturing, Parser, StringInput, Syntax, TsConfig};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::file_system::{is_declaration_file, Extensions};

pub fn parse_file(path: &PathBuf, visitor: &mut dyn VisitMut) {
    let cm: Lrc<SourceMap> = Default::default();
    let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

    let fm = cm
        .load_file(path)
        .expect(std::format!("Failed to load file {}", path.display()).as_str());

    let extension = path.extension().unwrap().to_str().unwrap();

    let lexer = Lexer::new(
        Syntax::Typescript(TsConfig {
            tsx: extension == Extensions::TSX || extension == Extensions::JSX,
            decorators: true,
            dts: is_declaration_file(&path),
            no_early_errors: false,
            disallow_ambiguous_jsx_like: extension == Extensions::MTS
                || extension == Extensions::CTS
                || extension == Extensions::MJS
                || extension == Extensions::CJS,
        }),
        EsVersion::latest(),
        StringInput::from(&*fm),
        None,
    );

    let capturing = Capturing::new(lexer);

    let mut parser = Parser::new_from(capturing);

    for e in parser.take_errors() {
        e.into_diagnostic(&handler).emit();
    }

    let module = parser
        .parse_typescript_module()
        .map_err(|e| e.into_diagnostic(&handler).emit())
        .expect("Failed to parse module.");

    let mut program = Program::Module(module);
    program.visit_mut_with(visitor);
}
