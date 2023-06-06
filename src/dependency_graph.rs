use clean_path::Clean;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::{collections::HashMap, path::PathBuf};

use crate::{
    cache::{DependencyGraphCache, Module, ModuleID},
    file_system::Extensions,
    tsconfig::TSConfig,
};

struct EdgesIter<'a> {
    module_edges: &'a Vec<ModuleID>,
    from: usize,
    to: usize,
}
impl<'a> Iterator for EdgesIter<'a> {
    type Item = ModuleID;

    fn next(&mut self) -> Option<ModuleID> {
        if self.from < self.to {
            self.from += 1;
            return Some(self.module_edges[self.from - 1]);
        } else {
            return None;
        }
    }
}

type ImportResolutionErrors = HashMap<PathBuf, Vec<String>>;

type ModuleIdx = usize;
pub struct DependencyGraph {
    module_ids: Vec<ModuleID>,
    module_id_to_idx: HashMap<ModuleID, ModuleIdx>,
    module_edge_offsets: Vec<usize>,
    module_edges: Vec<ModuleID>,

    dependency_graph_cache: DependencyGraphCache,
}
impl DependencyGraph {
    pub fn new(paths: &Vec<PathBuf>, tsconfig: &TSConfig) -> Self {
        let capacity = paths.len();

        return DependencyGraph {
            module_ids: Vec::with_capacity(capacity),
            module_id_to_idx: HashMap::with_capacity(capacity),
            module_edge_offsets: vec![],
            module_edges: vec![],

            dependency_graph_cache: DependencyGraphCache::new(&paths, &tsconfig),
        };
    }

    pub fn resolve_imports(
        &mut self,
        raw_dependencies: &Vec<(&PathBuf, Vec<PathBuf>)>,
    ) -> Option<ImportResolutionErrors> {
        struct ResolutionError {
            module: Module,
            message: String,
        }

        // tracks the resolution errors we encounter
        let resolution_errors: Mutex<Vec<ResolutionError>> = Mutex::new(vec![]);

        let mut resolved_dependencies = Vec::with_capacity(raw_dependencies.len());
        raw_dependencies
            .par_iter()
            .map(|(owner_path, dependencies)| {
                let owner = self.dependency_graph_cache.try_get_module_for_path(&owner_path).expect("A module should have already been defined");
                let parent = owner_path.parent().expect("Path should not be the root");

                let resolved_dependencies_for_module = dependencies.par_iter()
                    .filter_map(|dependency| {
                        if let Some(extension) = dependency.extension() {
                            // TODO(bradzacher) - we will want to track these eventually so we can understand that
                            //                    changes to these file types will cause changes to the importing JS
                            match extension.to_str().unwrap() {
                                Extensions::AVIF |
                                Extensions::CSS |
                                Extensions::EJS |
                                Extensions::FRAG |
                                Extensions::GIF |
                                Extensions::HTML |
                                Extensions::JPG |
                                Extensions::JSON |
                                Extensions::M4A |
                                Extensions::MD |
                                Extensions::MP3 |
                                Extensions::MP4 |
                                Extensions::OGV |
                                Extensions::OTF |
                                Extensions::PNG |
                                Extensions::SVG |
                                Extensions::TTF |
                                Extensions::TXT |
                                Extensions::VERT |
                                Extensions::VTT |
                                Extensions::WASM |
                                Extensions::WEBM |
                                Extensions::WOFF |
                                Extensions::WOFF2 => {
                                    return None;
                                },
                                _ => {}
                            }
                        }

                        if dependency.starts_with("../") || dependency.starts_with("./") {
                            // dependency is a relative reference which we must resolve relative to the owner file
                            let resolved_dependency_path = parent.join(dependency).clean();
                            if let Some(resolved_dependency) = self.dependency_graph_cache.try_get_module_for_path(&resolved_dependency_path) {
                                return Some(resolved_dependency.module_id.to_owned());
                            }

                            resolution_errors.lock().push(ResolutionError {
                                module: owner,
                                message: format!(
                                    "Unable to resolve relative import \"{}\" to an existing module, tried \"{}\"",
                                    dependency.display(),
                                    resolved_dependency_path.display(),
                                )
                            });
                            return None;
                        }

                        // check if it exists as-is in the module map
                        if let Some(existing_dep) = self.dependency_graph_cache.try_get_module_for_path(dependency) {
                            return Some(existing_dep.module_id.to_owned());
                        }

                        // assume it's a new, never before seen node_module and assign a new ModuleID for it

                        // note that we don't care about deep imports and just want the top-level node module name
                        // eg we don't care that `A -> mod/foo` and `B -> mod/bar`, we just care that `(A, B) -> mod`

                        let new_node_module = self.dependency_graph_cache.add_node_module(dependency);
                        return Some(new_node_module.module_id.to_owned());
                    }).collect::<Vec<_>>();

                return (owner.module_id.to_owned(), resolved_dependencies_for_module);
            })
            .collect_into_vec(&mut resolved_dependencies);

        // collect the errors
        let resolution_errors = {
            let resolution_errors = resolution_errors.lock();
            if resolution_errors.len() == 0 {
                None
            } else {
                let mut grouped: ImportResolutionErrors = HashMap::new();
                for error in resolution_errors.iter() {
                    let module_path = self
                        .dependency_graph_cache
                        .get_path_for_module(&error.module);
                    if let Some(errors) = grouped.get_mut(&module_path) {
                        errors.push(error.message.clone());
                    } else {
                        grouped.insert(module_path.clone(), vec![error.message.clone()]);
                    }
                }
                Some(grouped)
            }
        };

        // add the resolved dependency graph to the backing store
        for (module_id, edges) in resolved_dependencies {
            self.add_module(module_id, edges);
        }

        return resolution_errors;
    }

    fn add_module(&mut self, module_id: ModuleID, edges: Vec<ModuleID>) {
        self.module_id_to_idx
            .insert(module_id, self.module_ids.len());
        self.module_ids.push(module_id);
        self.module_edge_offsets.push(self.module_edges.len());
        for edge_id in edges {
            self.module_edges.push(edge_id.to_owned());
        }
    }

    // returning an iterator to avoid extra allocations of Vecs/Sets
    // fn get_children_for_id(&self, id: &ModuleID) -> impl Iterator<Item = ModuleID> + '_ {
    //     let idx = *self.module_id_to_idx.get(&id).unwrap();
    //     let from = self.module_edge_offsets[idx];
    //     let to = self
    //         .module_edge_offsets
    //         .get(idx + 1)
    //         .copied()
    //         .unwrap_or_else(|| self.module_edges.len());

    //     return EdgesIter {
    //         from,
    //         to,
    //         module_edges: &self.module_edges,
    //     };
    // }

    // pub fn get_all_dependencies(&self, path: &PathBuf) {
    //     let ids_to_search = vec![self.get_id_for_path(path)];
    //     let found_ids: HashSet<&u32> = HashSet::new();

    //     while ids_to_search.len() > 0 {
    //         let current_id = ids_to_search.pop();
    //         if current_id.contains(value) {}
    //     }
    // }
}

// impl fmt::Debug for DependencyGraph {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let mut dependency_map = HashMap::with_capacity(self.path_to_module_id_map.len());
//         for (path, id) in self.path_to_module_id_map {
//             dependency_map.insert(
//                 path.display().to_string(),
//                 self
//                     .get_children_for_id(&id)
//                     .map(|child_id| self.)
//                     .collect::<Vec<String>>(),
//             );
//         }

//         return f
//             .debug_struct("DependencyGraph")
//             .field("module_ids", &self.module_ids)
//             .field("module_id_to_idx", &self.module_id_to_idx)
//             .field("module_edge_offsets", &self.module_edge_offsets)
//             .field("module_edges", &self.module_edges)
//             .field("path_to_module_id_map", &self.path_to_module_id_map)
//             .finish();
//     }
// }
