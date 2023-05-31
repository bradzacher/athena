use clean_path::Clean;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

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

    pub fn add_dependency(&mut self, owner: &'map PathBuf, dependencies: HashSet<PathBuf>) {
        let mut cleaned_dependencies: HashSet<PathBuf> = HashSet::new();
        for dependency in dependencies {
            cleaned_dependencies.insert(
                if dependency.starts_with("../") || dependency.starts_with("./") {
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
                },
            );
        }

        self.files_to_dependencies
            .insert(owner, cleaned_dependencies);
    }
}
