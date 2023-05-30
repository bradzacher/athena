use swc_ecma_ast::{ExportAll, Import, ImportDecl, NamedExport, TsImportEqualsDecl};
use swc_ecma_visit::VisitMut;

pub struct ImportVisitor;
impl VisitMut for ImportVisitor {
    // import('bar');
    fn visit_mut_import(&mut self, _expr: &mut Import) {
        // TODO
        // println!("visit_mut_import");
    }

    // import foo from 'bar';
    fn visit_mut_import_decl(&mut self, _expr: &mut ImportDecl) {
        // TODO
        // println!("visit_mut_import_decl");
    }

    // import foo = require('bar');
    fn visit_mut_ts_import_equals_decl(&mut self, _expr: &mut TsImportEqualsDecl) {
        // TODO
        // println!("visit_mut_ts_import_equals_decl");
    }

    // export * from 'bar';
    fn visit_mut_export_all(&mut self, _expr: &mut ExportAll) {
        // TODO
        // println!("visit_mut_export_all");
    }

    // export { foo }; we ignore this case
    // export { foo } from 'bar';
    fn visit_mut_named_export(&mut self, _expr: &mut NamedExport) {
        // TODO
        // println!("visit_mut_export_named");
    }
}
