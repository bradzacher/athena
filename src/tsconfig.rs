use clean_path::Clean;
use json_comments::StripComments;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

// This obviously isn't the entire TSConfig spec - we only declare the subsets we actually care about
#[derive(Deserialize)]
#[serde(untagged)]
enum TSConfigExtends {
    Single(String),
    Variadic(Vec<String>),
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TSConfigCompilerOptions {
    base_url: Option<String>,
    paths: Option<HashMap<String, Vec<String>>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TSConfigRaw {
    extends: Option<TSConfigExtends>,
    compiler_options: Option<TSConfigCompilerOptions>,
}

#[derive(Default, Debug)]
pub struct TSConfig {
    pub base_url: Option<PathBuf>,
    pub paths: Option<HashMap<String, PathBuf>>,
}

pub fn parse_tsconfig(base_path: &Path) -> TSConfig {
    let raw_json_with_comments = std::fs::read_to_string(base_path)
        .expect(&format!("Unable to read tsconfig {}", base_path.display()));
    let raw_json = StripComments::new(raw_json_with_comments.as_bytes());
    let tsconfig_raw: TSConfigRaw = serde_json::from_reader(raw_json)
        .expect(&format!("Unable to parse tsconfig {}", base_path.display()));

    let base_path_parent = base_path.parent().expect("Path should not be the root");
    let mut base_tsconfig = match tsconfig_raw.compiler_options {
        None => TSConfig::default(),
        Some(compiler_options) => {
            let base_url = &compiler_options.base_url;
            TSConfig {
                base_url: match base_url {
                    Some(base_url) => Some(base_path_parent.join(base_url).clean()),
                    None => None,
                },
                paths: match compiler_options.paths {
                    Some(paths) => {
                        let base = match base_url {
                            Some(p) => PathBuf::from_str(&p).expect("Expected a valid path"),
                            None => base_path_parent.to_path_buf(),
                        };
                        Some(
                            paths
                                .iter()
                                .map(|(k, v)| {
                                    match v.len() {
                                        0 => {
                                            panic!("Found no path mappings for path key {}", k);
                                        },
                                        1 => {
                                            return (k.to_owned(), base.join(&v[0]).clean());
                                        }
                                        _ => {
                                            panic!("Multiple mapping paths is not currently supported for key {}", k);
                                        }
                                    }
                                })
                                .collect::<HashMap<String, PathBuf>>(),
                        )
                    }
                    None => None,
                },
            }
        }
    };

    if let Some(extends) = tsconfig_raw.extends {
        match extends {
            TSConfigExtends::Single(parent_path) => {
                let parent_path = if parent_path.starts_with("./") || parent_path.starts_with("../")
                {
                    base_path
                        .parent()
                        .expect("Should not be the root")
                        .join(parent_path)
                        .clean()
                } else {
                    panic!("Extending a tsconfig from node_modules is not currently supported");
                };

                let parent_tsconfig = parse_tsconfig(&parent_path);
                match base_tsconfig.base_url {
                    None => base_tsconfig.base_url = parent_tsconfig.base_url,
                    Some(_) => {}
                }
                match base_tsconfig.paths {
                    None => base_tsconfig.paths = parent_tsconfig.paths,
                    Some(_) => {}
                }
            }
            TSConfigExtends::Variadic(_) => {
                panic!("Extending multiple tsconfigs is not currently supported");
            }
        }
    }

    return base_tsconfig;
}
