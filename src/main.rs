mod cli;
mod dependency_graph;
mod file_system;
mod import_visitor;
mod parser;

use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::cli::parse_cli;
use crate::dependency_graph::DependencyGraph;
use crate::file_system::get_files;
use crate::import_visitor::ImportVisitor;
use crate::parser::parse_file;

fn main() {
    let args = parse_cli();

    eprintln!("Getting file list...");
    let start = Instant::now();
    let files = get_files(args);
    let duration = start.elapsed();
    eprintln!("Found {} files in {:?}", files.len(), duration);

    eprintln!("Parsing and extracting dependencies...");
    let start = Instant::now();
    let graph = Arc::new(Mutex::new(DependencyGraph::new()));
    files.par_iter().for_each(|file| {
        let mut visitor = ImportVisitor::new();
        parse_file(file, &mut visitor);

        if !visitor.errors.is_empty() {
            eprintln!("Errors for file {}:\n{:#?}", file.display(), visitor.errors);
        }

        let graph = graph.clone();
        graph
            .lock()
            .unwrap()
            .add_dependency(file, visitor.dependencies);
    });
    let duration: std::time::Duration = start.elapsed();
    eprintln!("Done in {:?}!", duration);

    // println!("{:#?}", graph);
}
