mod cli;
mod logic;
mod ui;

use byte_unit::Byte;
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use either::Either;
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
    selected_partition: Option<(Either<usize, NewPartition>, TableState)>,
    input: Option<Input>,
}

impl State<'_> {
    pub fn real_partition_index(&self, device: usize, partition: usize) -> usize {
        partition
            - self.devices[device]
                .partitions_with_empty()
                .iter()
                .take(partition)
                .filter(|p| p.is_right())
                .count()
    }
}

fn as_left<T, U>(either: &Either<T, U>) -> Option<&T> {
    match either {
        Either::Left(l) => Some(l),
        Either::Right(_) => None,
    }
}

fn get_preceding(dev: &Device, bounds: &RangeInclusive<i64>) -> Byte {
    let prev_index = {
        let next_index = dev
            .partitions()
            .position(|p| p.bounds().end() > bounds.start())
            .unwrap_or_else(|| dev.partitions().count() - 1);
        next_index as i64 - 1
    };
    if prev_index < 0 {
        Byte::from_u64(0)
    } else {
        let prev_end = dev
            .partitions()
            .nth(prev_index as usize)
            .unwrap()
            .bounds()
            .end();
        Byte::from_u64((bounds.start() - prev_end - 1) as u64 * dev.sector_size())
    }
}
