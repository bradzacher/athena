mod cli;
mod file_system;
mod import_visitor;
mod parser;

use crate::cli::parse_cli;
use crate::file_system::get_files;
use crate::import_visitor::ImportVisitor;
use crate::parser::parse_file;

fn main() {
    let args = parse_cli();
    let files = get_files(args);

    for file in files.iter() {
        parse_file(file, &mut ImportVisitor);
    }

    println!("{} files parsed", files.len());
}
