mod cli;
mod logic;
mod ui;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use partner::{Device, FileSystem};
use ratatui::widgets::TableState;
use ratatui_elm::App;
use std::ops::RangeInclusive;
use tracing_subscriber::EnvFilter;
use tui_input::Input;

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
    let mut state = State {
        devices: Device::get_all().context("failed to get devices")?,
        selected_device: None,
        selected_partition: None,
        table: TableState::new().with_selected(Some(0)),
        input: None,
    };

    if let Some(device) = cli.device {
        if let Some(index) = state.devices.iter().position(|d| d.path() == device) {
            state.selected_device = Some(index);
        } else {
            state
                .devices
                .push(Device::open(device).context("failed to open device")?);

            state.selected_device = Some(state.devices.len() - 1);
        }
    }

    App::new_with(state, logic::update, ui::view).run()?;

    Ok(())
}

struct NewPartition {
    name: String,
    fs: FileSystem,
    bounds: RangeInclusive<i64>,
}

struct State<'a> {
    devices: Vec<Device<'a>>,
    table: TableState,
    selected_device: Option<usize>,
    selected_partition: Option<(OneOf<usize, NewPartition>, TableState)>,
    input: Option<Input>,
}

enum OneOf<T, U> {
    Left(T),
    Right(U),
}
