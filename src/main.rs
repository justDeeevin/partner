use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use partner::Device;
use ratatui::widgets::TableState;
use ratatui_elm::App;
use tracing_subscriber::EnvFilter;

mod cli;
mod logic;
mod ui;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();

    if !nix::unistd::Uid::effective().is_root() {
        return Err(eyre!("partner must be run as root"));
    }

    if let Some(path) = &cli.log_file {
        let file = std::fs::File::create(path).context("failed to create log file")?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    let mut devices = Device::get_all().context("failed to get devices")?;

    if let Some(device) = cli.device {
        devices.push(Device::open(device).context("failed to open device")?);
    }

    App::new_with(
        State {
            devices,
            selected_device: None,
            table: TableState::new().with_selected(0),
        },
        logic::update,
        ui::view,
    )
    .run()?;

    Ok(())
}

struct State<'a> {
    devices: Vec<Device<'a>>,
    selected_device: Option<usize>,
    table: TableState,
}
