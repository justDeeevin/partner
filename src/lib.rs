use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
};

use byte_unit::Byte;
use proc_mounts::MountInfo;

#[repr(transparent)]
pub struct Devices(Vec<Device>);

impl Deref for Devices {
    type Target = [Device];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Devices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Devices {
    pub fn get() -> std::io::Result<Self> {
        let mounts: HashMap<_, _> = proc_mounts::MountIter::new()?
            .flatten()
            .map(|m| (m.source.clone(), m))
            .collect();
        Ok(Self(
            libparted::Device::devices(true)
                .flat_map(|d| Device::from_libparted(d, &mounts))
                .collect(),
        ))
    }
}

pub struct Device {
    pub model: Arc<str>,
    pub path: Arc<Path>,
    pub size: Byte,
    pub partitions: Vec<Partition>,
    changes: Vec<Change>,
    libparted: libparted::Device<'static>,
}

impl Device {
    pub(crate) fn from_libparted(
        mut value: libparted::Device<'static>,
        mounts: &HashMap<PathBuf, MountInfo>,
    ) -> std::io::Result<Self> {
        let sector_size = value.sector_size();
        let partitions = libparted::Disk::new(&mut value)?
            .parts()
            .map(|p| {
                let mount_info = p.get_path().and_then(|p| mounts.get(p));
                Partition::from_libparted(p, sector_size, mount_info)
            })
            .collect();
        Ok(Self {
            model: value.model().into(),
            path: value.path().into(),
            size: Byte::from_u64(value.length() * sector_size),
            partitions,
            changes: Vec::new(),
            libparted: value,
        })
    }

    pub fn n_changes(&self) -> usize {
        self.changes.len()
    }

    pub fn apply_change(&mut self, change: Change) {
        self.changes.push(change);
    }

    pub fn undo_change(&mut self) {
        self.changes.pop();
    }

    pub fn undo_all_changes(&mut self) {
        self.changes.clear();
    }

    pub fn commit(&mut self) -> std::io::Result<()> {
        let mut disk = libparted::Disk::new(&mut self.libparted)?;

        for change in self.changes.drain(..) {
            change.apply(&mut disk)?;
        }

        disk.commit()
    }
}

#[derive(Debug, Clone)]
pub enum Change {
    ChangeName(),
}

impl Change {
    pub(crate) fn apply(self, disk: &mut libparted::Disk) -> std::io::Result<()> {
        match self {}
    }
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub name: Arc<str>,
    pub path: Option<Arc<Path>>,
    pub size: Byte,
    // TODO
    // pub used: Byte,
    pub mount_point: Option<Arc<Path>>,
}

impl Partition {
    pub fn unused(&self) -> bool {
        self.path.is_none()
    }

    pub fn mounted(&self) -> bool {
        self.mount_point.is_some()
    }

    pub(crate) fn from_libparted(
        value: libparted::Partition,
        sector_size: u64,
        mount_info: Option<&MountInfo>,
    ) -> Self {
        Self {
            name: value.name().unwrap_or_default().into(),
            path: value.get_path().map(Arc::from),
            size: Byte::from_u64(value.geom_length() as u64 * sector_size),
            mount_point: mount_info.map(|m| Arc::from(m.dest.as_ref())),
        }
    }
}
