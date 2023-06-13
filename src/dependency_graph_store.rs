use std::{
    cmp::Ordering,
    collections::HashMap,
    ffi::OsString,
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use crate::{
    file_system::{extensions, is_declaration_file},
    tsconfig::TSConfig,
};

pub type PathId = usize;

#[derive(Clone, Copy, Eq)]
pub struct Module {
    path_id: PathId,
    pub module_id: ModuleId,
}
impl PartialEq<Module> for Module {
    fn eq(&self, other: &Module) -> bool {
        return self.module_id == other.module_id;
    }
}
impl Hash for Module {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.module_id.hash(state);
    }
}

pub type ModuleId = usize;

pub struct DependencyGraphStore {
    path_id_to_path: Vec<PathBuf>,
    path_to_path_id: HashMap<PathBuf, PathId>,

    pub module_id_to_module: Vec<Module>,
    // note - we use a hashmap here on purpose. If this were a Vec, we'd need to keep its length in sync with
    // path_id_to_path - which would double the number of resizes we need and substantially slow things down!
    path_id_to_module: HashMap<PathId, Module>,
}
impl DependencyGraphStore {
    pub fn modules(&self) -> &Vec<Module> {
        return &self.module_id_to_module;
    }

    pub fn new(paths: &Vec<PathBuf>, tsconfig: &TSConfig) -> Self {
        let path_id_to_path = paths.iter().cloned().collect::<Vec<PathBuf>>();
        let path_to_path_id: HashMap<PathBuf, PathId> = paths
            .iter()
            .enumerate()
            .map(|(id, path)| (path.to_owned(), id))
            .collect::<HashMap<PathBuf, PathId>>();

        let module_id_to_module = path_id_to_path
            .iter()
            .enumerate()
            .map(|(id, _)| {
                return Module {
                    path_id: id,
                    module_id: id,
                };
            })
            .collect::<Vec<_>>();

        let path_id_to_module = module_id_to_module
            .iter()
            .map(|module| (module.path_id, module.clone()))
            .collect::<HashMap<PathId, Module>>();

        let mut module_cache = Self {
            path_id_to_path,
            path_to_path_id,
            module_id_to_module,
            path_id_to_module,
        };

        module_cache.resolve_paths(tsconfig);

        return module_cache;
    }
}

// Path cache
impl DependencyGraphStore {
    pub fn try_get_id_for_path(&self, path: &Path) -> Option<PathId> {
        return self
            .path_to_path_id
            .get(path)
            .and_then(|id| Some(id.to_owned()));
    }

    pub fn get_id_for_path(&mut self, path: &Path) -> PathId {
        if let Some(id) = self.path_to_path_id.get(path) {
            return id.to_owned();
        }

        let paths = &mut self.path_id_to_path;
        let new_id = paths.len();
        paths.push(path.to_owned());

        self.path_to_path_id.insert(path.to_owned(), new_id);

        return new_id;
    }

    pub fn get_path_for_id(&self, id: &PathId) -> PathBuf {
        return self.path_id_to_path[id.to_owned()].clone();
    }
}

// Module cache
impl DependencyGraphStore {
    fn resolve_paths(&mut self, tsconfig: &TSConfig) {
        let index_file_name = OsString::from_str("index").unwrap();

        // in order to save ourselves doing path resolution later we instead want to register every valid path for a
        // given module ahead-of-time. This front-loads the effort as much as possible to reduce duplicate transforms
        // done when resolving imported names.

        // TODO(bradzacher) - need to handle tsconfig paths
        // TODO(bradzacher) - ban base_url folders as node modules

        let path_to_potential_module_iter = self
            .module_id_to_module
            .iter()
            // First we generate all possible non-relative import names for each module
            .map(|module| {
                let path = self.get_path_for_id(&module.path_id);

                let mut extra_paths = vec![];

                if let Some(base_url) = &tsconfig.base_url {
                    if let Ok(path_without_base) = path.strip_prefix(base_url) {
                        extra_paths.push((path_without_base.to_path_buf(), module.clone()));
                    }
                }

                if path.file_stem().unwrap() == index_file_name {
                    // index files are importable via their parent folder name
                    extra_paths.push((
                        path.parent()
                            .expect("Should not be the parent")
                            .to_path_buf(),
                        module.clone(),
                    ))
                }

                // add extension-less variants for each of the extra paths
                for i in 0..extra_paths.len() {
                    let (extra_path, _) = &extra_paths[i];
                    extra_paths.push(
                        // extension-less version which is the standard way to import things
                        (get_path_without_extension(&extra_path), module.clone()),
                    );
                }
                // and an extension-less variant for the base path
                extra_paths.push((get_path_without_extension(&path), module.clone()));

                return extra_paths;
            })
            .flatten()
            // then we group the modules by path
            .fold(HashMap::new(), |mut acc, (path, module)| {
                acc.entry(path).or_insert(Vec::new()).push(module);
                return acc;
            });
        // we now have a map of {path -> potential modules} - next step we need to determine the best module in order to
        // be left with just one module per path.
        let best_module_per_path = path_to_potential_module_iter
            .iter()
            .map(|(path, modules)| {
                match modules.len() {
                    1 => {
                        return (path.to_owned(), modules[0]);
                    }
                    _ => {
                        // Note: sorting so highest precedence is first
                        let mut modules = modules.clone();
                        modules.sort_by(|a, b| -> Ordering {
                            let a_path = self.get_path_for_module(&a);
                            let b_path = self.get_path_for_module(&b);

                            // prefer /path/to/foo.ts over /path/to/foo/index.ts
                            if a_path.file_stem().unwrap() == index_file_name
                                && b_path.file_stem().unwrap() != index_file_name
                            {
                                return Ordering::Greater;
                            }
                            if a_path.file_stem().unwrap() != index_file_name
                                && b_path.file_stem().unwrap() == index_file_name
                            {
                                return Ordering::Less;
                            }

                            return get_extension_precedence(&b_path)
                                .cmp(&get_extension_precedence(&a_path));
                        });

                        return (path.to_owned(), modules[0]);
                    }
                };
            })
            .collect::<Vec<_>>();

        for (path, module) in best_module_per_path {
            let path_id = self.get_id_for_path(&path);
            self.path_id_to_module.insert(path_id, module.clone());
        }
    }

    pub fn add_node_module(&mut self, path: &Path) -> Module {
        // we just want the top-level node module name, not the deep path
        // eg we don't care that `A -> mod/foo` and `B -> mod/bar`, we just care that `(A, B) -> mod`
        let module_name = {
            let mut components = path.components();
            if path.starts_with("@") {
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
                                path.display()
                            );
                        }
                    }
                    _ => {
                        panic!("Invalid node module name {}", path.display())
                    }
                }
            } else {
                match components.next().expect("Expected a first part") {
                    Component::Normal(first) => PathBuf::from(first),
                    _ => {
                        panic!("Invalid node module name {}", path.display())
                    }
                }
            }
        };
        let module = self.get_module_for_path(&module_name);

        // for future lookups we also want to include the mapping from the deep import path
        let path_id = self.get_id_for_path(path);
        self.path_id_to_module.insert(path_id, module.clone());

        return module;
    }

    pub fn get_path_for_module(&self, module: &Module) -> PathBuf {
        return self.get_path_for_id(&module.path_id);
    }

    pub fn try_get_module_for_path(&self, path: &Path) -> Option<Module> {
        return self
            .path_id_to_module
            .get(&self.try_get_id_for_path(path)?)
            .and_then(|m| Some(m.clone()));
    }

    fn get_module_for_path(&mut self, path: &Path) -> Module {
        let path_id = self.get_id_for_path(path);

        if let Some(module) = self.path_id_to_module.get(&path_id) {
            return module.clone();
        }

        let new_id = self.module_id_to_module.len();
        self.module_id_to_module.push(Module {
            path_id,
            module_id: new_id,
        });
        let module = &self.module_id_to_module[new_id];

        self.path_id_to_module.insert(path_id, module.clone());

        return module.clone();
    }

    pub fn get_module_for_id(&self, id: &ModuleId) -> Module {
        return self.module_id_to_module[id.to_owned()].clone();
    }
}

fn get_extension_precedence(path: &Path) -> u8 {
    let mut extension = path.extension().unwrap().to_str().unwrap();
    if is_declaration_file(path) {
        if extension == extensions::TS {
            extension = extensions::D_TS;
        } else if extension == extensions::CTS {
            extension = extensions::D_CTS;
        } else if extension == extensions::MTS {
            extension = extensions::D_MTS;
        }
    }

    // https://github.com/microsoft/TypeScript/blob/f0ff97611f2e9c8aff208f4b6520489fe387e9ab/src/compiler/utilities.ts#L9171
    // ['ts', 'tsx', 'd.ts', 'js', 'jsx', 'cts', 'd.cts', 'cjs', 'mts', 'd.mts', 'mjs']
    return match extension {
        extensions::TS => 11,
        extensions::TSX => 10,
        extensions::D_TS => 9,
        extensions::JS => 8,
        extensions::JSX => 7,
        extensions::CTS => 6,
        extensions::D_CTS => 5,
        extensions::CJS => 4,
        extensions::MTS => 3,
        extensions::D_MTS => 2,
        extensions::MJS => 1,
        _ => 0,
    };
}

fn get_path_without_extension(path: &Path) -> PathBuf {
    if is_declaration_file(&path) {
        // you don't include the `.d`
        return path.with_extension("").with_extension("");
    } else {
        return path.with_extension("");
    }
}
