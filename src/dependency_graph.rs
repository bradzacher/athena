use clean_path::Clean;
use parking_lot::Mutex;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    Direction,
};
use rayon::prelude::*;
use spliter::ParallelSpliterator;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::PathBuf,
};

use crate::{
    dependency_graph_store::{DependencyGraphStore, Module, ModuleID},
    depth_first_expansion::DepthFirstExpansion,
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

struct GraphData {
    graph: DiGraph<ModuleID, ModuleID>,
    module_id_to_node_idx: Vec<NodeIndex>,
}

pub struct DependencyGraph {
    dependency_graph_store: DependencyGraphStore,
    graph_data: Option<GraphData>,
}
impl DependencyGraph {
    pub fn new(paths: &Vec<PathBuf>, tsconfig: &TSConfig) -> Self {
        let dependency_graph_store = DependencyGraphStore::new(&paths, &tsconfig);

        return DependencyGraph {
            graph_data: None,
            dependency_graph_store,
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

        let resolved_dependencies: Vec<(ModuleID, ModuleID)> =
            raw_dependencies
                .par_iter()
                .map(|(owner_path, dependencies)| {
                    let owner = self.dependency_graph_store.try_get_module_for_path(&owner_path).expect("A module should have already been defined");
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
                                if let Some(resolved_dependency) = self.dependency_graph_store.try_get_module_for_path(&resolved_dependency_path) {
                                    return Some((owner.module_id.to_owned(), resolved_dependency.module_id.to_owned()));
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
                            if let Some(existing_dep) = self.dependency_graph_store.try_get_module_for_path(dependency) {
                                return Some((owner.module_id.to_owned(), existing_dep.module_id.to_owned()));
                            }

                            // assume it's a new, never before seen node_module and assign a new ModuleID for it

                            // note that we don't care about deep imports and just want the top-level node module name
                            // eg we don't care that `A -> mod/foo` and `B -> mod/bar`, we just care that `(A, B) -> mod`

                            let new_node_module = self.dependency_graph_store.add_node_module(dependency);
                            return Some((owner.module_id.to_owned(), new_node_module.module_id.to_owned()));
                        }).collect::<Vec<(ModuleID, ModuleID)>>();

                    return resolved_dependencies_for_module;
                })
                .flatten()
                .collect::<Vec<_>>();

        // collect the errors
        let resolution_errors = {
            let resolution_errors = resolution_errors.lock();
            if resolution_errors.len() == 0 {
                None
            } else {
                let mut grouped: ImportResolutionErrors = HashMap::new();
                for error in resolution_errors.iter() {
                    let module_path = self
                        .dependency_graph_store
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

        // add the resolved dependency graph to the backing graph
        let modules = self.dependency_graph_store.modules().read();
        let module_count = modules.len();
        let mut graph: DiGraph<ModuleID, ModuleID> =
            DiGraph::with_capacity(module_count, resolved_dependencies.len());
        let mut module_id_to_node_idx = Vec::with_capacity(modules.len());
        for module in modules.iter() {
            module_id_to_node_idx.insert(module.module_id, graph.add_node(module.module_id));
        }
        for (from_id, to_id) in resolved_dependencies {
            graph.add_edge(
                module_id_to_node_idx[from_id],
                module_id_to_node_idx[to_id],
                0,
            );
        }
        self.graph_data = Some(GraphData {
            graph,
            module_id_to_node_idx,
        });

        return resolution_errors;
    }

    pub fn get_all_dependencies(
        &self,
        path: &PathBuf,
        direction: Direction,
        max_depth: u32,
    ) -> Result<HashSet<PathBuf>, GetDependenciesError> {
        let graph_data = self.graph_data.as_ref().ok_or(GetDependenciesError::new(
            "Cannot call get_all_dependencies before resolve_imports",
        ))?;

        let module_id = self
            .dependency_graph_store
            .try_get_module_for_path(&path)
            .ok_or(GetDependenciesError::new(""))?
            .module_id;

        let node_idx = graph_data.module_id_to_node_idx[module_id];
        let dfe = DepthFirstExpansion::new(&graph_data.graph, direction, max_depth, node_idx);

        let paths = dfe
            .par_split()
            .map(|node_idx| {
                let module_id = graph_data.graph.node_weight(node_idx).unwrap();
                self.dependency_graph_store
                    .get_path_for_module(&self.dependency_graph_store.get_module_for_id(&module_id))
            })
            .fold(HashSet::new, |mut acc, path| {
                acc.insert(path);
                return acc;
            })
            .reduce(HashSet::new, |mut a, b| {
                a.extend(b);
                return a;
            });

        return Ok(paths);
    }
}

pub struct GetDependenciesError<'a> {
    message: &'a str,
}
impl<'a> GetDependenciesError<'a> {
    pub fn new(message: &'a str) -> Self {
        return Self { message };
    }
}
impl<'a> fmt::Debug for GetDependenciesError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return fmt::Debug::fmt(&self.message, f);
    }
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
