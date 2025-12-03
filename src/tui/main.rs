mod cli;
mod logic;
mod ui;

use byte_unit::Byte;
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use partner::{Device, FileSystem, Partition};
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

impl State<'_> {
    pub fn real_partition_index(&self, device: usize, partition: usize) -> usize {
        partition
            - partitions_with_empty(&self.devices[device])
                .iter()
                .take(partition)
                .filter(|p| p.is_right())
                .count()
    }
}

fn partitions_with_empty<'a>(dev: &'a Device) -> Vec<OneOf<&'a Partition, RangeInclusive<i64>>> {
    let mut partitions = dev.partitions().map(OneOf::Left).collect::<Vec<_>>();
    if !partitions.is_empty() {
        let mut i = 0;
        if *partitions[0].left().unwrap().bounds().start() > 1 {
            partitions.insert(
                0,
                OneOf::Right(1..=partitions[0].left().unwrap().bounds().start() - 1),
            );
            i += 1;
        }
        while i < partitions.len() - 1 {
            let left = *partitions[i].left().unwrap().bounds().end();
            let right = *partitions[i + 1].left().unwrap().bounds().start();
            assert!(right > left, "overlapping partitions");
            if right - left > 1 {
                partitions.insert(i + 1, OneOf::Right(left + 1..=right - 1));
                i += 1;
            }

            i += 1;
        }
        let end = *partitions
            .last()
            .and_then(|p| match p {
                OneOf::Left(p) => Some(p),
                OneOf::Right(_) => None,
            })
            .unwrap()
            .bounds()
            .end();
        if Byte::from_u64(end as u64 * dev.sector_size()) < dev.size() {
            partitions.push(OneOf::Right(
                end..=(dev.size().as_u64() / dev.sector_size()) as i64,
            ));
        }
    }

    partitions
}

enum OneOf<T, U> {
    Left(T),
    Right(U),
}

impl<T, U> OneOf<T, U> {
    pub fn left(&self) -> Option<&T> {
        match self {
            OneOf::Left(l) => Some(l),
            OneOf::Right(_) => None,
        }
    }

    pub fn is_right(&self) -> bool {
        matches!(self, OneOf::Right(_))
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
