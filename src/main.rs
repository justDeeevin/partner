use color_eyre::Result;

mod cli;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();

    Ok(())
}
