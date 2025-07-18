use color_eyre::{Result, eyre::Context};
use libparted::Device;
use ratatui::widgets::TableState;

mod cli;
mod ext;
mod logic;
mod ui;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();

    let mut devices = Device::devices(true).collect::<Vec<_>>();
    let mode = if let Some(device) = cli.device {
        if !devices.iter().any(|d| d.path() == device) {
            devices.push(Device::new(device).context("Failed to open specified device")?);
        }
        Mode::Partitions(devices.len() - 1)
    } else {
        Mode::Disks
    };

    let state = State {
        table: TableState::default().with_selected(0),
        mode,
        devices,
        actions: Vec::new(),
    };

    ratatui_elm::App::new_with(state, logic::update, ui::view).run()?;
    Ok(())
}

enum Action {}

struct State {
    pub table: TableState,
    pub mode: Mode,
    pub devices: Vec<Device<'static>>,
    pub actions: Vec<Action>,
}

enum Mode {
    Disks,
    Partitions(usize),
}
