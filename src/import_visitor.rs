use std::path::PathBuf;

use swc_atoms::js_word;
use swc_ecma_ast::{
    CallExpr, ExportAll, Expr, ImportDecl, Lit, NamedExport, TsImportType, TsModuleRef,
};
use swc_ecma_visit::VisitMut;

use crate::dependency_graph::DependencyGraph;

pub struct ImportVisitor<'visitor, 'graph> {
    dependency_graph: &'visitor mut DependencyGraph<'graph>,
    file_path: &'graph PathBuf,
    pub errors: Vec<String>,
}
impl<'visitor, 'graph> ImportVisitor<'visitor, 'graph> {
    pub fn new(
        file_path: &'graph PathBuf,
        dependency_graph: &'visitor mut DependencyGraph<'graph>,
    ) -> ImportVisitor<'visitor, 'graph> {
        return ImportVisitor {
            dependency_graph,
            file_path,
            errors: vec![],
        };
    }

    fn get_dependency_for_call_like_expr(&mut self, kind: &str, expr: &mut CallExpr) {
        if expr.args.len() != 1 {
            self.errors.push(format!(
                "Expected a `{}` with exactly 1 string argument, found {} arguments - in file {}",
                kind,
                expr.args.len(),
                self.file_path.display()
            ));
        } else {
            match &*expr.args[0].expr {
                Expr::Lit(literal) => match literal {
                    Lit::Str(str) => self
                        .dependency_graph
                        .add_dependency_jsword(self.file_path, &str.value),
                    default => {
                        self.errors.push(format!(
                            "Expected a `{}` with exactly 1 string argument, found 1 {:?} arguments - in file {}",
                            kind,
                            default,
                            self.file_path.display()
                        ));
                    }
                },
                Expr::Ident(_) => {
                    self.errors.push(format!(
                        "Found a dynamic `{}`, unable to resolve dependency - in file {}",
                        kind,
                        self.file_path.display()
                    ));
                }
                default => {
                    self.errors.push(format!(
                        "Expected a `{}` with exactly 1 string argument, found 1 {:?} arguments - in file {}",
                        kind,
                        default,
                        self.file_path.display()
                    ));
                }
            }
        }
    }
}
impl<'visitor, 'graph> VisitMut for ImportVisitor<'visitor, 'graph> {
    // type T = import('a');
    fn visit_mut_ts_import_type(&mut self, expr: &mut TsImportType) {
        self.dependency_graph
            .add_dependency_jsword(self.file_path, &expr.arg.value);
    }

    // import foo from 'bar';
    fn visit_mut_import_decl(&mut self, expr: &mut ImportDecl) {
        self.dependency_graph
            .add_dependency_jsword(self.file_path, &expr.src.value);
    }

    // import foo = ...;
    fn visit_mut_ts_module_ref(&mut self, expr: &mut TsModuleRef) {
        match expr {
            // import foo = foo.bar; -- we ignore this case
            TsModuleRef::TsEntityName(_) => (),
            // import foo = require('bar');
            //              ^^^^^^^^^^^^^^
            TsModuleRef::TsExternalModuleRef(module_ref) => self
                .dependency_graph
                .add_dependency_jsword(self.file_path, &module_ref.expr.value),
        }
    }

    // export * from 'bar';
    fn visit_mut_export_all(&mut self, expr: &mut ExportAll) {
        self.dependency_graph
            .add_dependency_jsword(self.file_path, &expr.src.value);
    }

    // export { foo }; -- we ignore this case
    // export { foo } from 'bar';
    fn visit_mut_named_export(&mut self, expr: &mut NamedExport) {
        match &expr.src {
            Some(src) => self
                .dependency_graph
                .add_dependency_jsword(self.file_path, &src.value),
            None => (
                // no source means it's local name only
            ),
        }
    }

    fn visit_mut_call_expr(&mut self, expr: &mut CallExpr) {
        match &expr.callee {
            swc_ecma_ast::Callee::Import(_) => {
                // import('foo')
                self.get_dependency_for_call_like_expr("import", expr);
            }
            swc_ecma_ast::Callee::Expr(callee) => match &**callee {
                Expr::Ident(ident) => {
                    if ident.sym == js_word!("require") {
                        // require('foo')
                        self.get_dependency_for_call_like_expr("require", expr);
                    }
                }
                _ => (
                    // random require which we ignore
                ),
            },
            swc_ecma_ast::Callee::Super(_) => (
                // super call which we ignore
            ),
        }
    }
}
