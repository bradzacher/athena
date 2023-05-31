use clean_path::Clean;
use ignore::{types::TypesBuilder, WalkBuilder};
use std::{env, path::PathBuf};

use crate::cli::CliArgs;

pub fn get_files(args: CliArgs) -> Vec<PathBuf> {
    let mut types_builder = TypesBuilder::new();
    types_builder
        .add("typescript", "*.{cts,mts,ts,tsx}")
        .expect("Invalid glob");
    types_builder.select("typescript");
    types_builder
        .add("javascript", "*.{cjs,mjs,js,jsx}")
        .expect("Invalid glob");
    types_builder.select("javascript");
    let types = types_builder.build().expect("Unable to build types");

    let mut walk_builder = WalkBuilder::new(absolutize(PathBuf::from(&args.paths[0])));
    if args.paths.len() > 1 {
        for path in args.paths.iter().skip(1) {
            walk_builder.add(absolutize(PathBuf::from(&path)));
        }
    }
    walk_builder.types(types);

    let mut files: Vec<PathBuf> = vec![];

    for result in walk_builder.build() {
        // Each item yielded by the iterator is either a directory entry or an
        // error, so either handle the path or the error.
        match result {
            Ok(entry) => match entry.file_type() {
                Some(file_type) => {
                    if file_type.is_dir() {
                        continue;
                    }
                    files.push(entry.path().to_owned().clean());
                }
                None => {
                    continue;
                }
            },
            Err(err) => println!("ERROR: {}", err),
        };
    }

    return files;
}

pub fn absolutize(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    return env::current_dir()
        .expect("Could not access current directory")
        .join(path);
}
