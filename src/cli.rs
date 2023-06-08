use clap::{builder::ValueParser, Parser, ValueEnum};
use petgraph::Direction;
use std::path::PathBuf;

use crate::file_system::path_parser_absolute;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum CliDirection {
    /// Get the dependencies (outgoing imports) of the given module
    Dependencies = 0,
    /// Get the dependents (incoming imports) of the given module
    Dependents = 1,
}
impl Into<Direction> for CliDirection {
    fn into(self) -> Direction {
        return match self {
            Self::Dependents => Direction::Incoming,
            Self::Dependencies => Direction::Outgoing,
        };
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// The paths to search for files
    #[arg(required = true, num_args = 1.., value_parser = ValueParser::new(path_parser_absolute))]
    pub search_paths: Vec<PathBuf>,

    /// The path to a tsconfig file to resolve `paths` and
    #[arg(long, short = 'p', required = true, value_parser = ValueParser::new(path_parser_absolute))]
    pub tsconfig_path: PathBuf,

    #[arg(long, short, value_parser = ValueParser::new(path_parser_absolute))]
    pub file: Option<PathBuf>,

    #[arg(value_enum, long, short, default_value_t = CliDirection::Dependencies)]
    pub direction: CliDirection,

    #[arg(long)]
    pub dump_resolved_imports: Option<PathBuf>,
}

pub fn parse_cli() -> CliArgs {
    return CliArgs::parse();
}
