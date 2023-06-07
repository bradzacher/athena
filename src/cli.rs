use clap::{builder::ValueParser, Parser, ValueEnum};
use petgraph::Direction;
use std::path::PathBuf;

use crate::file_system::path_parser_absolute;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum CliDirection {
    /// An `Outgoing` edge is an outward edge *from* the current node. AKA Dependencies
    Outgoing = 0,
    /// An `Incoming` edge is an inbound edge *to* the current node. AKA Dependents
    Incoming = 1,
}
impl Into<Direction> for CliDirection {
    fn into(self) -> Direction {
        return match self {
            Self::Incoming => Direction::Incoming,
            Self::Outgoing => Direction::Outgoing,
        };
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(required = true, num_args = 1.., value_parser = ValueParser::new(path_parser_absolute))]
    pub search_paths: Vec<PathBuf>,

    #[arg(long, short = 'p', required = true, value_parser = ValueParser::new(path_parser_absolute))]
    pub tsconfig_path: PathBuf,

    #[arg(long, short, value_parser = ValueParser::new(path_parser_absolute))]
    pub file: Option<PathBuf>,

    #[arg(value_enum, long, short)]
    pub direction: Option<CliDirection>,
}

pub fn parse_cli() -> CliArgs {
    return CliArgs::parse();
}
