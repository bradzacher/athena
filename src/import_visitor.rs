use std::{collections::HashSet, path::PathBuf, str::FromStr};

use swc_atoms::{js_word, JsWord};
use swc_ecma_ast::{
    CallExpr, ExportAll, Expr, ImportDecl, Lit, NamedExport, TsImportType, TsModuleRef,
};
use swc_ecma_visit::VisitMut;

pub struct ImportVisitor {
    pub errors: Vec<String>,
    pub dependencies: HashSet<PathBuf>,
}
impl ImportVisitor {
    pub fn new() -> ImportVisitor {
        return ImportVisitor {
            errors: vec![],
            dependencies: HashSet::new(),
        };
    }

    fn add_dependency(&mut self, dependency: &JsWord) {
        self.dependencies
            .insert(PathBuf::from_str(dependency).expect("Expected a valid path"));
    }

    fn get_dependency_for_call_like_expr(&mut self, kind: &str, expr: &mut CallExpr) {
        if expr.args.len() != 1 {
            self.errors.push(format!(
                "Expected a `{}` with exactly 1 string argument, found {} arguments",
                kind,
                expr.args.len(),
            ));
        } else {
            match &*expr.args[0].expr {
                Expr::Lit(literal) => match literal {
                    Lit::Str(str) => self.add_dependency(&str.value),
                    default => {
                        self.errors.push(format!(
                            "Expected a `{}` with exactly 1 string argument, found 1 {:?} arguments",
                            kind,
                            default,
                        ));
                    }
                },
                Expr::Ident(_) => {
                    self.errors.push(format!(
                        "Found a dynamic `{}`, unable to resolve dependency",
                        kind,
                    ));
                }
                default => {
                    self.errors.push(format!(
                        "Expected a `{}` with exactly 1 string argument, found 1 {:?} arguments",
                        kind, default,
                    ));
                }
            }
        }
    }
}
impl VisitMut for ImportVisitor {
    // type T = import('a');
    fn visit_mut_ts_import_type(&mut self, expr: &mut TsImportType) {
        self.add_dependency(&expr.arg.value);
    }

    // import foo from 'bar';
    fn visit_mut_import_decl(&mut self, expr: &mut ImportDecl) {
        self.add_dependency(&expr.src.value);
    }

    // import foo = ...;
    fn visit_mut_ts_module_ref(&mut self, expr: &mut TsModuleRef) {
        match expr {
            // import foo = foo.bar; -- we ignore this case
            TsModuleRef::TsEntityName(_) => {}
            // import foo = require('bar');
            //              ^^^^^^^^^^^^^^
            TsModuleRef::TsExternalModuleRef(module_ref) => {
                self.add_dependency(&module_ref.expr.value)
            }
        }
    }

    // export * from 'bar';
    fn visit_mut_export_all(&mut self, expr: &mut ExportAll) {
        self.add_dependency(&expr.src.value);
    }

    // export { foo } from 'bar';
    fn visit_mut_named_export(&mut self, expr: &mut NamedExport) {
        match &expr.src {
            Some(src) => self.add_dependency(&src.value),
            None => {
                // export { foo }; -- we ignore this case
            }
        }
    }

    // import('foo')
    // require('foo')
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
                _ => {
                    // random call expression which we ignore
                }
            },
            swc_ecma_ast::Callee::Super(_) => {
                // super call which we ignore
            }
        }
    }
}
