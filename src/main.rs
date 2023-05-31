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
    let files = get_files(args);

    println!("Calculating dependency graph...");
    let mut graph = DependencyGraph::new();
    for file in files.iter() {
        let mut visitor = ImportVisitor::new(file, &mut graph);
        parse_file(file, &mut visitor);
    }

    println!("{} files parsed", files.len());

    println!("{:#?}", graph);
}
