use std::{
    collections::HashMap,
    panic,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
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

    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        orig_hook(panic_info);
        std::process::exit(1);
    }));

    let worker = std::thread::spawn(worker);

    let mode = rx_mode
        .blocking_recv()
        .unwrap()
        .context("Failed to open specified device")?;

    tx_actions.send(Action::GetDevices).unwrap();
    if let Mode::Partitions { index, .. } = mode {
        tx_actions.send(Action::SetDisk(index)).unwrap();
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
        n_changes: 0,
    };

    ratatui_elm::App::new_with(state, logic::update, ui::view)
        .subscription(UnboundedReceiverStream::new(rx_messages))
        .run()?;

    worker.join().unwrap();

    Ok(())
}

#[derive(Clone)]
enum Action {
    GetDevices,
    SetDisk(usize),
    ChangeName {
        partition: usize,
        previous_name: Option<Arc<str>>,
        new_name: Arc<str>,
    },
    Undo,
    Commit,
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
    pub n_changes: usize,
}

impl State {
    pub fn action(&mut self, action: Action) {
        match action {
            Action::GetDevices | Action::SetDisk(_) => {}
            Action::Undo => {
                self.n_changes -= 1;
            }
            Action::Commit => self.n_changes = 0,
            _ => self.n_changes += 1,
        }
        self.tx_actions.send(action).unwrap();
    }
}

#[derive(Debug)]
enum Mode {
    Disks,
    Partitions {
        index: usize,
        temp_name: Option<String>,
    },
}

impl Mode {
    pub const fn partitions(index: usize) -> Self {
        Self::Partitions {
            index,
            temp_name: None,
        }
    }

    pub fn is_editing_name(&self) -> bool {
        matches!(
            self,
            Mode::Partitions {
                temp_name: Some(_),
                ..
            }
        )
    }
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

        let mut changes = Vec::new();

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
            tx_mode.send(Ok(Mode::partitions(index))).unwrap();
        } else {
            tx_mode.send(Ok(Mode::Disks)).unwrap();
        }

        let mut disk = None;
        while let Ok(action) = rx_actions.recv() {
            if !matches!(
                action,
                Action::Undo | Action::GetDevices | Action::SetDisk(_)
            ) {
                changes.push(action.clone());
            }
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
                                            name: p.name().map(Into::into),
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
                Action::ChangeName {
                    partition: index,
                    new_name,
                    previous_name: _,
                } => {
                    match disk
                        .as_mut()
                        .unwrap()
                        .get_partition(index as u32)
                        .unwrap()
                        .set_name(&new_name)
                    {
                        Ok(()) => {}
                        Err(e) => {
                            let _ = tx_messages.send(Message::Error(e.into()));
                        }
                    }
                }
                Action::Undo => {
                    let Some(action) = changes.pop() else {
                        continue;
                    };
                    match action {
                        Action::GetDevices | Action::SetDisk(_) | Action::Undo | Action::Commit => {
                        }
                        Action::ChangeName {
                            partition,
                            new_name: _,
                            previous_name,
                        } => {
                            match disk
                                .as_mut()
                                .unwrap()
                                .get_partition(partition as u32)
                                .unwrap()
                                .set_name(&previous_name.unwrap_or_default())
                            {
                                Ok(()) => {}
                                Err(e) => {
                                    let _ = tx_messages.send(Message::Error(e.into()));
                                }
                            }
                        }
                    }
                }
                Action::Commit => {
                    changes.clear();
                    disk.as_mut().unwrap().commit().unwrap();
                }
            }
        }
    };
    (rx_mode, worker)
}
