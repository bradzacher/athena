mod cli;
mod dependency_graph;
mod dependency_graph_store;
mod depth_first_expansion;
mod file_system;
mod import_visitor;
mod parser;
mod tsconfig;

use petgraph::Direction;
use rayon::prelude::*;
use std::io;
use std::time::Instant;

use crate::cli::parse_cli;
use crate::dependency_graph::DependencyGraph;
use crate::file_system::{get_files, path_parser_absolute};
use crate::import_visitor::ImportVisitor;
use crate::parser::parse_file;
use crate::tsconfig::parse_tsconfig;

/// Simple macro to measure the time taken for an expression
macro_rules! measure {
    ($e:expr) => {{
        let start = Instant::now();
        let result = { $e };
        let duration = start.elapsed();
        (result, duration)
    }};
    ($start_label:literal, $e:expr) => {{
        eprintln!($start_label);
        let start = Instant::now();
        let result = { $e };
        let duration = start.elapsed();
        (result, duration)
    }};
}
macro_rules! print_timer {
    ($fmt:literal, $($arg:tt)*) => {{
        let fmt = format!($fmt, $($arg)*);
        eprintln!("⏲️  {}\n", &fmt);
    }};
}

fn main() {
    let (graph, duration) = measure!("Preparing dependency graph", {
        let args = parse_cli();

        let (tsconfig, duration) =
            measure!("Parsing tsconfig...", parse_tsconfig(&args.tsconfig_path));
        print_timer!("Parsed in {:?}", duration);

        let (files, duration) = measure!("Getting file list...", get_files(&args.paths));
        print_timer!("Found {} files in {:?}", files.len(), duration);

        let mut raw_dependencies = Vec::with_capacity(files.len());
        let (_, duration) = measure!(
            "Parsing and extracting dependencies...",
            files
                .par_iter()
                .map(|file| {
                    let mut visitor = ImportVisitor::new();
                    parse_file(file, &mut visitor);

                    if !visitor.errors.is_empty() {
                        eprintln!("❌ Errors for file {}:", file.display());
                        for error in visitor.errors {
                            eprintln!("❗️ {}", error);
                        }
                        eprintln!();
                    }

                    return (file, visitor.dependencies);
                })
                .collect_into_vec(&mut raw_dependencies)
        );
        print_timer!("Done in {:?}", duration);

        let (mut graph, duration) = measure!(
            "Preparing path -> module ID map",
            DependencyGraph::new(&files, &tsconfig)
        );
        print_timer!("Done in {:?}", duration);

        let (resolution_errors, duration) = measure!(
            "Resolving import strings and building dependency graph",
            graph.resolve_imports(&raw_dependencies)
        );
        if let Some(resolution_errors) = resolution_errors {
            for (file, errors) in resolution_errors.iter() {
                eprintln!("❌ Errors for file {}:", file.display());
                for error in errors {
                    eprintln!("❗️ {}", error);
                }
                eprintln!();
            }
        }
        print_timer!("Done in {:?}", duration);

        graph
    });
    print_timer!("Graph built in {:?}", duration);

    loop {
        println!("Enter file path (relative or absolute):");
        let file_input = match read_line() {
            Some(l) => l,
            None => return,
        };

        let direction: Direction;
        loop {
            println!("Enter direction - dependencies (0) or dependents (1):");
            direction = match read_line() {
                Some(l) => match l.as_str() {
                    "0" | "dependencies" => Direction::Outgoing,
                    "1" | "dependents" => Direction::Incoming,
                    _ => continue,
                },
                None => return,
            };
            break;
        }

        match path_parser_absolute(&file_input) {
            Ok(file) => {
                let (maybe_dependencies, duration) = measure!(
                    "Fetching dependencies",
                    graph.get_all_dependencies(&file, direction)
                );
                match maybe_dependencies {
                    Ok(dependencies) => {
                        print_timer!("Done in {:?}:\n{:#?}", duration, dependencies)
                    }
                    Err(e) => println!("Error getting dependencies {:?}", e),
                }
            }
            Err(e) => {
                println!("Invalid path: {}", e);
            }
        }

        println!();
    }
}

fn read_line<'a>() -> Option<String> {
    let mut line = String::new();
    io::stdin().read_line(&mut line).expect("Valid input");
    let line = line.trim();
    if line == "q" {
        return None;
    }
    return Some(line.to_owned());
}
