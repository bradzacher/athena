use clap::Parser as ClapParser;

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(required = true, num_args = 1..)]
    pub paths: Vec<String>,
}

pub fn parse_cli() -> CliArgs {
    return CliArgs::parse();
}
