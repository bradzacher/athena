use clean_path::Clean;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    Direction,
};
use rayon::prelude::*;
use spliter::ParallelSpliterator;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    dependency_graph_store::DependencyGraphStore,
    depth_first_expansion::DepthFirstExpansion,
    file_system::extensions,
    module::{EdgeWeight, Module, ModuleGraph, ModuleId},
    tsconfig::TSConfig,
};

type ImportResolutionErrors = HashMap<PathBuf, Vec<String>>;

// these two pieces of data are intrinsically linked and will either both exist or not exist
// hence they sit on a separate struct, rather than directly on DependencyGraph
struct GraphData {
    graph: ModuleGraph,
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

    fn resolve_dependencies_for_module(
        &mut self,
        resolution_errors: &mut Vec<ResolutionError>,
        owner_path: &PathBuf,
        dependencies: &Vec<PathBuf>,
    ) -> Vec<(ModuleId, ModuleId)> {
        let owner = self
            .dependency_graph_store
            .try_get_module_for_path(&owner_path)
            .expect("A module should have already been defined");
        let parent = owner_path.parent().expect("Path should not be the root");

        let resolved_dependencies_for_module = dependencies.iter()
            .filter_map(|dependency| {
                if let Some(extension) = dependency.extension() {
                    // TODO(bradzacher) - we will want to track these eventually so we can understand that
                    //                    changes to these file types will cause changes to the importing JS
                    match extension.to_str().unwrap() {
                        extensions::AVIF |
                        extensions::CSS |
                        extensions::EJS |
                        extensions::FRAG |
                        extensions::GIF |
                        extensions::HTML |
                        extensions::JPG |
                        extensions::JSON |
                        extensions::M4A |
                        extensions::MD |
                        extensions::MP3 |
                        extensions::MP4 |
                        extensions::OGV |
                        extensions::OTF |
                        extensions::PNG |
                        extensions::SVG |
                        extensions::TTF |
                        extensions::TXT |
                        extensions::VERT |
                        extensions::VTT |
                        extensions::WASM |
                        extensions::WEBM |
                        extensions::WOFF |
                        extensions::WOFF2 => {
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

                    resolution_errors.push(ResolutionError {
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
            }).collect::<Vec<_>>();

        return resolved_dependencies_for_module;
    }

    pub fn resolve_imports(
        &mut self,
        raw_dependencies: &[(&PathBuf, Vec<PathBuf>)],
    ) -> Option<ImportResolutionErrors> {
        // tracks the resolution errors we encounter
        let mut resolution_errors: Vec<ResolutionError> = vec![];

        let resolved_dependencies: Vec<(ModuleId, ModuleId)> = raw_dependencies
            .iter()
            .map(|(owner_path, dependencies)| {
                return self.resolve_dependencies_for_module(
                    &mut resolution_errors,
                    owner_path,
                    dependencies,
                );
            })
            .flatten()
            .collect();

        // collect the errors
        let resolution_errors = {
            if resolution_errors.is_empty() {
                None
            } else {
                let mut grouped: ImportResolutionErrors = HashMap::new();
                for error in resolution_errors.iter() {
                    let module_path = self
                        .dependency_graph_store
                        .get_path_for_module(&error.module);
                    grouped
                        .entry(module_path.clone())
                        .or_insert(Vec::new())
                        .push(error.message.clone());
                }
                Some(grouped)
            }
        };

        // add the resolved dependency graph to the backing graph
        let modules = self.dependency_graph_store.modules();
        let module_count = modules.len();
        let mut graph: ModuleGraph =
            DiGraph::with_capacity(module_count, resolved_dependencies.len());
        let mut module_id_to_node_idx = Vec::with_capacity(modules.len());
        for module in modules.iter() {
            module_id_to_node_idx.insert(module.module_id.into(), graph.add_node(module.module_id));
        }
        for (from_id, to_id) in resolved_dependencies {
            graph.add_edge(
                module_id_to_node_idx[from_id],
                module_id_to_node_idx[to_id],
                EdgeWeight,
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
        path: &Path,
        direction: Direction,
        max_depth: u32,
    ) -> Result<HashSet<PathBuf>, &str> {
        let graph_data = self
            .graph_data
            .as_ref()
            .ok_or("Cannot call get_all_dependencies before resolve_imports")?;

        let module_id = self
            .dependency_graph_store
            .try_get_module_for_path(&path)
            .ok_or("Unable to get module for path")?
            .module_id;

        let node_idx = graph_data.module_id_to_node_idx[module_id];
        let dfe = DepthFirstExpansion::new(&graph_data.graph, direction, max_depth, node_idx);

        let paths = dfe
            .par_split()
            .map(|node_idx| {
                let module_id = graph_data.graph.node_weight(node_idx).unwrap();
                self.dependency_graph_store
                    .get_path_for_module(&self.dependency_graph_store.get_module_for_id(*module_id))
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

struct ResolutionError {
    module: Module,
    message: String,
}
