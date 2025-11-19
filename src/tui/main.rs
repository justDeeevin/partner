mod cli;
mod logic;
mod ui;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use partner::Device;
use ratatui::widgets::TableState;
use ratatui_elm::App;
use std::{collections::BTreeMap, path::Path, sync::Arc};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();

    if !nix::unistd::Uid::effective().is_root() {
        return Err(eyre!("partner must be run as root"));
    }

    if cli.debug {
        let file = std::fs::File::create("partner.log").context("failed to create log file")?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    let devices = Device::get_all()
        .context("failed to get devices")?
        .into_iter()
        .map(|d| (d.path_owned(), d))
        .collect();
    let mut state = State {
        devices,
        selected_device: None,
        table: TableState::new().with_selected(Some(0)),
    };

    if let Some(device) = cli.device {
        let device = Arc::from(device);
        if !state.devices.contains_key(&device) {
            state.devices.insert(
                device.clone(),
                Device::open(&device).context("failed to get device")?,
            );
        }
        state.selected_device = Some(device.clone());
    }

    App::new_with(state, logic::update, ui::view).run()?;

    Ok(())
}

struct State<'a> {
    devices: BTreeMap<Arc<Path>, Device<'a>>,
    selected_device: Option<Arc<Path>>,
    table: TableState,
}
