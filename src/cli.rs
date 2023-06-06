use clap::{builder::ValueParser, Parser};
use std::path::PathBuf;

use crate::file_system::path_parser_absolute;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(required = true, num_args = 1.., value_parser = ValueParser::new(path_parser_absolute))]
    pub paths: Vec<PathBuf>,

    #[arg(long, short = 'p', required = true, value_parser = ValueParser::new(path_parser_absolute))]
    pub tsconfig_path: PathBuf,
}

pub fn parse_cli() -> CliArgs {
    return CliArgs::parse();
}
