use color_eyre::Result;

mod cli;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();

    let devices = partner::Device::get_all()?;

    if let Some(device) = cli.device {
        let device = devices.iter().find(|d| d.path() == device).unwrap();
        dbg!(device);
    } else {
        dbg!(devices);
    }

    Ok(())
}
