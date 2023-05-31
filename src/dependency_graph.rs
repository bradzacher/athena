use clean_path::Clean;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
};

use swc_atoms::JsWord;

#[derive(Debug)]
pub struct DependencyGraph<'map> {
    files_to_dependencies: HashMap<&'map PathBuf, HashSet<PathBuf>>,
}
impl<'map> DependencyGraph<'map> {
    pub fn new() -> DependencyGraph<'map> {
        return DependencyGraph {
            files_to_dependencies: HashMap::new(),
        };
    }

    pub fn add_dependency_jsword(&mut self, owner: &'map PathBuf, dependency: &JsWord) {
        self.add_dependency(
            owner,
            PathBuf::from_str(dependency).expect("Expected a valid path"),
        );
    }
    pub fn add_dependency(&mut self, owner: &'map PathBuf, dependency: PathBuf) {
        let resolved_dependency = if dependency.starts_with("../") || dependency.starts_with("./") {
            // dependency is a relative reference which we must resolve relative to the owner file
            owner
                .parent()
                .expect("Path should not be the root")
                .join(dependency)
                .clean()
        } else {
            // path is a tsconfig path or a node_module
            // TODO
            dependency
        };

        let dependency_set: &mut HashSet<PathBuf> = match self.files_to_dependencies.get_mut(owner)
        {
            Some(set) => set,
            None => {
                self.files_to_dependencies.insert(owner, HashSet::new());
                self.files_to_dependencies
                    .get_mut(owner)
                    .expect("Key must exist")
            }
        };
        dependency_set.insert(resolved_dependency);
    }
}
