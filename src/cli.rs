use clap::{builder::ValueParser, Parser};
use std::{path::PathBuf, str::FromStr};

/// Ensures a path argument exists and converts it to an absolute representation
fn path_parser_absolute(path: &str) -> Result<PathBuf, std::io::Error> {
    return PathBuf::from_str(path)
        .expect(&format!("Expected a valid path, got {}", path))
        .canonicalize();
}

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
