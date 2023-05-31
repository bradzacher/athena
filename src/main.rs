mod cli;
mod dependency_graph;
mod file_system;
mod import_visitor;
mod parser;

use crate::cli::parse_cli;
use crate::dependency_graph::DependencyGraph;
use crate::file_system::get_files;
use crate::import_visitor::ImportVisitor;
use crate::parser::parse_file;

fn main() {
    let args = parse_cli();

    eprintln!("Getting file list...");
    let files = get_files(args);
    eprintln!("Found {} files", files.len());

    eprintln!("Calculating dependency graph...");
    let mut graph = DependencyGraph::new();
    for file in files.iter() {
        let mut visitor = ImportVisitor::new(file, &mut graph);
        parse_file(file, &mut visitor);
        if !visitor.errors.is_empty() {
            eprintln!("Errors for file {}:\n{:#?}", file.display(), visitor.errors);
        }
    }
    eprintln!("Done!");

    println!("{:#?}", graph);
}
