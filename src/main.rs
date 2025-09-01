use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};

use crate::ext::{DeviceInfo, PartitionInfo};
use byte_unit::Byte;
use color_eyre::{Result, eyre::Context};
use libparted::{Device, Disk};
use proc_mounts::{MountInfo, MountList};
use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;

mod cli;
mod ext;
mod logic;
mod ui;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();

    let (tx_actions, rx_actions) = std::sync::mpsc::channel();
    let (tx_messages, rx_messages) = tokio::sync::mpsc::unbounded_channel();

    let (rx_mode, worker) = worker(rx_actions, tx_messages, cli.device);

    std::thread::spawn(worker);

    let mode = rx_mode
        .blocking_recv()
        .unwrap()
        .context("Failed to open specified device")?;

    tx_actions.send(Action::GetDevices).unwrap();
    if let Mode::Partitions(i) = mode {
        tx_actions.send(Action::SetDisk(i)).unwrap();
    }

    let state = State {
        table: TableState::default().with_selected(0),
        mode,
        tx_actions,
        devices: Vec::new(),
        partitions: Vec::new(),
        mounts: MountList::new()
            .context("Failed to get mounts")?
            .0
            .into_iter()
            .map(|m| (m.source.clone(), m))
            .collect(),
    };

    ratatui_elm::App::new_with(state, logic::update, ui::view)
        .subscription(UnboundedReceiverStream::new(rx_messages))
        .run()?;
    Ok(())
}

enum Action {
    GetDevices,
    SetDisk(usize),
    DeleteAll,
}

enum Message {
    Partitions(Vec<PartitionInfo>),
    Devices(Vec<DeviceInfo>),
    Error(color_eyre::Report),
}

struct State {
    pub table: TableState,
    pub mode: Mode,
    pub tx_actions: Sender<Action>,
    pub devices: Vec<DeviceInfo>,
    pub partitions: Vec<PartitionInfo>,
    pub mounts: HashMap<PathBuf, MountInfo>,
}

#[derive(Debug)]
enum Mode {
    Disks,
    Partitions(usize),
}

fn worker(
    rx_actions: Receiver<Action>,
    tx_messages: UnboundedSender<Message>,
    specified: Option<PathBuf>,
) -> (
    tokio::sync::oneshot::Receiver<std::io::Result<Mode>>,
    impl FnOnce(),
) {
    let (tx_mode, rx_mode) = tokio::sync::oneshot::channel();

    let worker = move || {
        let mut devices = Device::devices(true).collect::<Vec<_>>();

        if let Some(device) = specified {
            let index = match devices.iter().position(|d| d.path() == device) {
                Some(i) => i,
                None => {
                    match Device::new(device) {
                        Ok(d) => devices.push(d),
                        Err(e) => {
                            tx_mode.send(Err(e)).unwrap();
                            return;
                        }
                    }
                    devices.len() - 1
                }
            };
            tx_mode.send(Ok(Mode::Partitions(index))).unwrap();
        } else {
            tx_mode.send(Ok(Mode::Disks)).unwrap();
        }

        let mut disk = None;
        while let Ok(action) = rx_actions.recv() {
            match action {
                Action::GetDevices => {
                    disk = None;
                    tx_messages
                        .send(Message::Devices(
                            devices.iter().map(DeviceInfo::from).collect(),
                        ))
                        .unwrap();
                }
                Action::SetDisk(i) => {
                    disk = None;
                    let sector_size = devices[i].sector_size() as i64;
                    match Disk::new(&mut devices[i]) {
                        Ok(d) => {
                            tx_messages
                                .send(Message::Partitions(
                                    d.parts()
                                        .skip(1)
                                        .take(d.parts().count() - 2)
                                        .map(|p| PartitionInfo {
                                            path: p.get_path().map(Into::into),
                                            fs_type: p.fs_type_name().map(Into::into),
                                            length: Byte::from_i64(p.geom_length() * sector_size)
                                                .unwrap(),
                                        })
                                        .collect(),
                                ))
                                .unwrap();
                            disk = Some(d);
                        }
                        Err(e) => {
                            let _ = tx_messages.send(Message::Error(e.into()));
                        }
                    }
                }
                Action::DeleteAll => {
                    if let Err(e) = disk.as_mut().unwrap().delete_all() {
                        tx_messages.send(Message::Error(e.into())).unwrap();
                    }
                }
            }
        }
    };
    (rx_mode, worker)
}
