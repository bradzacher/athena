use clean_path::Clean;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    path::{Component, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{file_system::is_declaration_file, tsconfig::TSConfig};

type ModuleID = u32;
type ModuleIdx = usize;

struct ResolutionError {
    file: String,
    message: String,
}

struct EdgesIter<'a> {
    node_edges: &'a Vec<ModuleID>,
    from: usize,
    to: usize,
}
impl<'a> Iterator for EdgesIter<'a> {
    type Item = ModuleID;

    fn next(&mut self) -> Option<ModuleID> {
        if self.from < self.to {
            self.from += 1;
            Some(self.node_edges[self.from - 1])
        } else {
            None
        }
    }
}

pub struct DependencyGraph {
    module_ids: Vec<ModuleID>,
    module_id_to_idx: HashMap<ModuleID, ModuleIdx>,
    module_edge_offsets: Vec<usize>,
    module_edges: Vec<ModuleID>,
    path_to_module_id_map: HashMap<PathBuf, ModuleID>,
}
impl DependencyGraph {
    pub fn new(files: &Vec<PathBuf>) -> Self {
        let capacity = files.len();
        return DependencyGraph {
            module_ids: Vec::with_capacity(capacity),
            module_id_to_idx: HashMap::with_capacity(capacity),
            module_edge_offsets: vec![],
            module_edges: vec![],
            path_to_module_id_map: files
                .iter()
                .zip(0..files.len() as ModuleID)
                .map(|(f, i)| {
                    vec![
                        (f.to_owned(), i),
                        // we include a copy without the extension because it's common practice in non node-esm files
                        // to not include the extension and have it "just work"
                        (
                            if is_declaration_file(&f) {
                                // you don't include
                                f.with_extension("").with_extension("")
                            } else {
                                f.with_extension("")
                            },
                            i,
                        ),
                        // TODO(bradzacher) - maybe we want to add another alias if the filename is "index"?
                        //                    eg /path/to/foo/index.ts => /path/to/foo
                        //                    will need to also first check if there is an existing module with the name
                        //                    eg /path/to/foo.ts always takes precedence
                    ]
                })
                .flatten()
                .collect::<HashMap<PathBuf, ModuleID>>(),
        };
    }

    pub fn resolve_imports(
        &mut self,
        tsconfig: TSConfig,
        raw_dependencies: Vec<(&PathBuf, Vec<PathBuf>)>,
    ) -> (
        Option<HashMap<String, Vec<String>>>,
        Vec<(ModuleID, Vec<ModuleID>)>,
    ) {
        let current_module_id_max = self.path_to_module_id_map.len();

        // tracks the true node module names we'll add to "path_to_module_id_map" after the parallel iteration
        let node_module_deps: Arc<Mutex<HashMap<PathBuf, ModuleID>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // maps the possibly deep import paths for node modules to the module IDs we've defined in `node_module_deps`
        let node_module_import_path_to_node_module_id: Arc<Mutex<HashMap<&PathBuf, ModuleID>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // tracks the resolution errors we encounter
        let mut resolution_errors_raw: Vec<ResolutionError> = vec![];
        let resolution_errors: Arc<Mutex<&mut Vec<ResolutionError>>> =
            Arc::new(Mutex::new(&mut resolution_errors_raw));

        let mut resolved_dependencies: Vec<(u32, Vec<u32>)> = Vec::new();
        raw_dependencies
            .par_iter()
            .map(|(owner_path, dependencies)| {
                let owner_id = self
                    .path_to_module_id_map
                    .get(&owner_path as &PathBuf)
                    .expect("Expected and existing module path");
                let parent = owner_path.parent().expect("Path should not be the root");

                let resolved_dependencies = dependencies
                    .par_iter()
                    .filter_map(|dependency| {
                        // TODO(bradzacher) - we will want to track these eventually so we can understand that
                        //                    changes to these file types will cause changes to the importing JS
                        if let Some(extension) = dependency.extension() {
                            if extension == "css"
                                || extension == "ejs"
                                || extension == "html"
                                || extension == "json"
                                || extension == "svg"
                                || extension == "txt"
                                || extension == "wasm"
                                || extension == "gif"
                                || extension == "jpg"
                                || extension == "png"
                                || extension == "avif"
                                || extension == "mp4"
                                || extension == "mp3"
                                || extension == "ogv"
                                || extension == "webm"
                                || extension == "vert"
                                || extension == "frag"
                                || extension == "woff"
                            {
                                return None;
                            }
                        }

                        if dependency.starts_with("../") || dependency.starts_with("./") {
                            // dependency is a relative reference which we must resolve relative to the owner file
                            let resolved_dependency_path = parent.join(dependency).clean();
                            if let Some(resolved_dependency) = self
                            .path_to_module_id_map
                            .get(&resolved_dependency_path) {
                                return Some(resolved_dependency.to_owned());
                            }

                            resolution_errors.lock().unwrap().push(ResolutionError {
                                file: owner_path.display().to_string(),
                                message: format!(
                                    "Unable to resolve relative import \"{}\" to an existing module, tried \"{}\"",
                                    dependency.display(),
                                    resolved_dependency_path.display(),
                                )
                            });
                            return None;
                        }

                        if let Some(existing_dep) = self.path_to_module_id_map.get(dependency) {
                            return Some(existing_dep.to_owned());
                        }

                        // path is a tsconfig path or a node_module...

                        // first attempt to resolve against the baseUrl
                        if let Some(base_url) = &tsconfig.base_url {
                            let dependency_against_base_url = base_url.join(dependency).clean();
                            if let Some(dependency_module_id) =
                                self.path_to_module_id_map.get(&dependency_against_base_url)
                            {
                                return Some(dependency_module_id.to_owned());
                            }
                        }

                        // next attempt to resolve it against the new node_modules
                        let node_module_import_path_to_node_module_id =
                            node_module_import_path_to_node_module_id.clone();
                        if let Some(node_module_dep_id) = node_module_import_path_to_node_module_id
                            .lock()
                            .unwrap()
                            .get(dependency)
                        {
                            return Some(node_module_dep_id.to_owned());
                        }

                        // assume it's a new, never before seen node_module and assign a new ModuleID for it

                        // note that we don't care about deep imports and just want the top-level node module name
                        // eg we don't care that `A -> mod/foo` and `B -> mod/bar`, we just care that `(A, B) -> mod`
                        let mut components = dependency.components();
                        let module_name = if dependency.starts_with("@") {
                            // is an @-scoped name, which always has two parts
                            match components.next().expect("Expected a first part") {
                                Component::Normal(first) => {
                                    let first = PathBuf::from(first);
                                    if let Component::Normal(second) =
                                        components.next().expect("Expected a second part")
                                    {
                                        first.join(second)
                                    } else {
                                        panic!(
                                            "Unexpected component in node module name {}",
                                            dependency.display()
                                        );
                                    }
                                }
                                _ => {
                                    panic!("Invalid node module name {}", dependency.display())
                                }
                            }
                        } else {
                            match components.next().expect("Expected a first part") {
                                Component::Normal(first) => PathBuf::from(first),
                                _ => {
                                    panic!("Invalid node module name {}", dependency.display())
                                }
                            }
                        };

                        let node_module_deps = node_module_deps.clone();
                        let mut node_module_deps = node_module_deps.lock().unwrap();
                        let new_module_id =
                            (current_module_id_max + node_module_deps.len()).to_owned() as ModuleID;
                        node_module_deps.insert(module_name, new_module_id);
                        node_module_import_path_to_node_module_id
                            .lock()
                            .unwrap()
                            .insert(dependency, new_module_id);

                        return Some(new_module_id.to_owned());
                    })
                    .collect::<Vec<_>>();
                return (owner_id.to_owned(), resolved_dependencies);
            })
            .collect_into_vec(&mut resolved_dependencies);

        let resolution_errors = if resolution_errors_raw.len() == 0 {
            None
        } else {
            let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
            for error in resolution_errors_raw {
                if let Some(errors) = grouped.get_mut(&error.file) {
                    errors.push(error.message);
                } else {
                    grouped.insert(error.file, vec![error.message]);
                }
            }
            Some(grouped)
        };

        return (resolution_errors, resolved_dependencies);
    }

    fn add_module(&mut self, module_id: ModuleID, edges: Vec<ModuleID>) {
        self.module_id_to_idx
            .insert(module_id, self.module_ids.len());
        self.module_ids.push(module_id);
        self.module_edge_offsets.push(self.module_edges.len());
        for edge_id in edges {
            self.module_edges.push(edge_id);
        }
    }

    pub fn add_all_modules(&mut self, resolved_dependencies: Vec<(ModuleID, Vec<ModuleID>)>) {
        for (module_id, edges) in resolved_dependencies {
            self.add_module(module_id, edges);
        }
    }

    // returning an iterator to avoid extra allocations of Vecs/Sets
    // fn get_children(&self, id: ModuleID) -> impl Iterator<Item = ModuleID> + '_ {
    //     let idx = *self.module_id_to_idx.get(&id).unwrap();
    //     let from = self.module_edge_offsets[idx];
    //     let to = self
    //         .module_edge_offsets
    //         .get(idx + 1)
    //         .copied()
    //         .unwrap_or_else(|| self.module_edges.len());
    //     EdgesIter {
    //         from,
    //         to,
    //         module_edges: &self.module_edges,
    //     }
    // }
}
