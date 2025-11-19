use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg()]
    /// The path to the device to use
    pub device: Option<PathBuf>,
    #[arg(long, short = 'D')]
    /// Path to log file
    pub debug: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}
