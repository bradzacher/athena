mod cache;
mod cli;
mod dependency_graph;
mod file_system;
mod import_visitor;
mod parser;
mod tsconfig;

use rayon::prelude::*;
use std::time::Instant;

use crate::cli::parse_cli;
use crate::dependency_graph::DependencyGraph;
use crate::file_system::get_files;
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
    let (_, duration) = measure!({
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

        // let (result, duration) = measure!(
        //     "Example query -> ",
        //     graph.get_children(&path_parser_absolute("../../work/canva/web/src/services/content_management/marketing/automation/content_management_domain_marketing_automation_proto.ts").unwrap())
        // );
        // print_timer!("Found dependencies in {:?}:", duration);
        // eprintln!("{:#?}", result.collect::<Vec<u32>>());

        // println!("{:#?}", graph);
    });

    eprintln!("Completed in {:?}", duration);
}
