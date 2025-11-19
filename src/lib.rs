#![deny(clippy::unwrap_used)]

use byte_unit::Byte;
use proc_mounts::MountInfo;
use std::{
    collections::HashMap,
    fmt::Debug,
    ops::{Bound, RangeBounds, RangeInclusive},
    path::{Path, PathBuf},
    sync::Arc,
};
use strum::{Display, EnumString};

type RawDevice<'a> = libparted::Device<'a>;

pub struct Device<'a> {
    model: Arc<str>,
    path: Arc<Path>,
    size: Byte,
    partitions: Vec<Partition>,
    changes: Vec<Change>,
    raw: RawDevice<'a>,
}

impl Debug for Device<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("model", &self.model)
            .field("path", &self.path)
            .field("size", &self.size)
            .field("partitions", &self.partitions().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("given bounds overlap with existing partition №{0}")]
    OverlapsExisting(usize),
}

impl<'a> Device<'a> {
    fn get_mounts() -> std::io::Result<HashMap<PathBuf, MountInfo>> {
        Ok(proc_mounts::MountIter::new()?
            .flatten()
            .map(|m| (m.source.clone(), m))
            .collect())
    }

    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::from_libparted(RawDevice::new(path)?, &Self::get_mounts()?)
    }

    pub fn get_all() -> std::io::Result<Vec<Self>> {
        let mounts = Self::get_mounts()?;

        RawDevice::devices(true)
            .map(|d| Device::from_libparted(d, &mounts))
            .collect()
    }

    fn from_libparted(
        mut value: RawDevice<'a>,
        mounts: &HashMap<PathBuf, MountInfo>,
    ) -> std::io::Result<Self> {
        let sector_size = value.sector_size();
        let disk = libparted::Disk::new(&mut value)?;
        let partitions = disk.parts().skip(1).collect::<Vec<_>>();
        let len = partitions.len();
        let partitions = partitions
            .into_iter()
            .take(len - 1)
            .map(|p| {
                let mount_info = p.get_path().and_then(|p| mounts.get(p));
                Partition::from_libparted(p, sector_size, mount_info)
            })
            .collect();
        drop(disk);
        Ok(Self {
            model: value.model().into(),
            path: value.path().into(),
            size: Byte::from_u64(value.length() * sector_size),
            partitions,
            changes: Vec::new(),
            raw: value,
        })
    }

    pub fn model(&self) -> &str {
        self.model.as_ref()
    }

    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub fn path_owned(&self) -> Arc<Path> {
        self.path.clone()
    }

    pub fn size(&self) -> Byte {
        self.size
    }

    pub fn partitions(&self) -> impl Iterator<Item = &Partition> {
        self.partitions
            .iter()
            .filter(|p| p.kind != PartitionKind::Hidden)
    }

    fn partitions_enum(&self) -> impl Iterator<Item = (usize, &Partition)> {
        self.partitions
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind != PartitionKind::Hidden)
    }

    pub fn n_changes(&self) -> usize {
        self.changes.len()
    }

    pub fn change_partition_name(&mut self, partition: usize, new: Arc<str>) {
        self.partitions[partition].name.1.push(new.clone());
        self.changes.push(Change::Name { partition, new });
    }

    pub fn new_partition(
        &mut self,
        name: Arc<str>,
        fs: Option<FileSystem>,
        bounds: impl RangeBounds<i64>,
    ) -> Result<(), Error> {
        let bounds = match bounds.start_bound() {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => b + 1,
            Bound::Unbounded => 0,
        }..=match bounds.end_bound() {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => b - 1,
            Bound::Unbounded => self.raw.length() as i64,
        };

        let containing_index = self
            .partitions_enum()
            .find(|(_, p)| {
                !p.used
                    && p.bounds().contains(bounds.start())
                    && p.bounds().contains(bounds.end())
            })
            .ok_or_else(|| {
                #[allow(clippy::unwrap_used, reason = "the only situation in which the above returns `None` is when there is at least one index for which this is true")]
                let index = self
                    .partitions
                    .iter()
                    .position(|p| {
                        !p.used
                            && (p.bounds().contains(bounds.start())
                                || p.bounds().contains(bounds.end()))
                    })
                    .unwrap();
                Error::OverlapsExisting(index)
            })?.0;

        let new_partition = Partition::new(
            name.clone(),
            bounds.clone(),
            fs,
            true,
            self.raw.sector_size(),
        );

        let index = if self.partitions[containing_index].bounds() == &bounds {
            if self.partitions[containing_index].kind == PartitionKind::Virtual {
                self.partitions[containing_index] = new_partition;
            } else {
                self.partitions[containing_index].kind = PartitionKind::Hidden;
                self.partitions.insert(containing_index, new_partition);
            }
            containing_index
        } else if self.partitions[containing_index].bounds().start() == bounds.start() {
            let containing_end = *self.partitions[containing_index].bounds().end();
            self.partitions[containing_index]
                .bounds
                .1
                .push(*bounds.end()..=containing_end);
            self.partitions.insert(containing_index, new_partition);
            containing_index
        } else if self.partitions[containing_index].bounds().end() == bounds.end() {
            let containing_start = *self.partitions[containing_index].bounds().start();
            self.partitions[containing_index]
                .bounds
                .1
                .push(containing_start..=*bounds.start());
            self.partitions.insert(containing_index, new_partition);
            containing_index
        } else {
            let containing_start = *self.partitions[containing_index].bounds().start();
            let containing_end = *self.partitions[containing_index].bounds().end();
            self.partitions[containing_index]
                .bounds
                .1
                .push(containing_start..=*bounds.start());
            self.partitions.insert(containing_index + 1, new_partition);
            self.partitions.insert(
                containing_index + 2,
                Partition::new(
                    "".into(),
                    *bounds.end()..=containing_end,
                    None,
                    false,
                    self.raw.sector_size(),
                ),
            );

            containing_index + 1
        };

        self.changes.push(Change::NewPartition {
            name,
            fs,
            bounds,
            index,
        });

        Ok(())
    }

    pub fn undo_change(&mut self) {
        match self.changes.pop() {
            Some(Change::Name { partition, .. }) => {
                self.partitions[partition].name.1.pop();
            }
            Some(Change::NewPartition { mut index, .. }) => {
                assert!(
                    self.partitions[index].kind == PartitionKind::Virtual,
                    "undo tried to remove a real partition"
                );
                if let Some(prev) = self.partitions.get_mut(index - 1)
                    && !prev.used
                {
                    if prev.kind == PartitionKind::Virtual {
                        self.partitions.remove(index - 1);
                        index -= 1;
                    } else {
                        prev.bounds.1.pop();
                    }
                }
                if let Some(next) = self.partitions.get_mut(index + 1) {
                    match next.kind {
                        PartitionKind::Virtual => {
                            self.partitions.remove(index + 1);
                        }
                        PartitionKind::Hidden => {
                            next.kind = PartitionKind::Real;
                        }
                        PartitionKind::Real => {
                            next.bounds.1.pop();
                        }
                    }
                }
                self.partitions.remove(index);
            }
            None => {}
        }
    }

    pub fn undo_all_changes(&mut self) {
        self.changes.clear();

        for partition in &mut self.partitions {
            partition.undo_all_changes();
        }

        self.partitions.retain(|p| p.kind != PartitionKind::Virtual);
        self.partitions
            .iter_mut()
            .filter(|p| p.kind == PartitionKind::Hidden)
            .for_each(|p| p.kind = PartitionKind::Real);
    }

    pub fn commit(&mut self) -> std::io::Result<()> {
        let mut disk = libparted::Disk::new(&mut self.raw)?;

        for change in self.changes.drain(..) {
            change.apply(&mut disk)?;
        }

        disk.commit()
    }
}

enum Change {
    Name {
        partition: usize,
        new: Arc<str>,
    },
    NewPartition {
        name: Arc<str>,
        fs: Option<FileSystem>,
        bounds: RangeInclusive<i64>,
        index: usize,
    },
}

#[derive(Display, EnumString, Debug, Clone, Copy)]
#[strum(serialize_all = "kebab-case")]
pub enum FileSystem {
    Btrfs,
    Exfat,
    Ext2,
    Ext4,
    F2fs,
    Fat16,
    Fat32,
    Jfs,
    #[strum(serialize = "linux-swap(v1)")]
    LinuxSwap,
    Ntfs,
    Xfs,
}

impl From<FileSystem> for libparted::FileSystemType<'_> {
    fn from(value: FileSystem) -> Self {
        #[allow(clippy::unwrap_used, reason = "statically impossible")]
        Self::get(&value.to_string()).unwrap()
    }
}

impl Change {
    fn apply(self, disk: &mut libparted::Disk) -> std::io::Result<()> {
        match self {
            #[allow(
                clippy::unwrap_used,
                reason = "a panic here would be an internal logic bug"
            )]
            Self::Name { partition, new } => disk
                .parts()
                .nth(partition + 1)
                .unwrap()
                .set_name(new.as_ref()),
            Self::NewPartition {
                name, fs, bounds, ..
            } => {
                let mut part = libparted::Partition::new(
                    disk,
                    libparted::PartitionType::PED_PARTITION_NORMAL,
                    fs.map(Into::into).as_ref(),
                    *bounds.start(),
                    *bounds.end(),
                )?;

                part.set_name(name.as_ref())?;

                disk.add_partition(
                    &mut part,
                    // SAFETY: this device reference is only used once
                    &unsafe { disk.get_device().get_optimal_aligned_constraint()? },
                )
            }
        }
    }
}

pub struct Partition {
    pub path: Option<Arc<Path>>,
    // TODO
    // pub occupied: Byte,
    pub mount_point: Option<Arc<Path>>,
    pub used: bool,
    kind: PartitionKind,
    name: (Arc<str>, Vec<Arc<str>>),
    bounds: (RangeInclusive<i64>, Vec<RangeInclusive<i64>>),
    fs: (Option<FileSystem>, Vec<Option<FileSystem>>),
    sector_size: u64,
}

impl Debug for Partition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Partition")
            .field("path", &self.path)
            .field("mount_point", &self.mount_point)
            .field("used", &self.used)
            .field("name", &self.name())
            .field("bounds", self.bounds())
            .field("fs", &self.fs())
            .finish()
    }
}

#[derive(PartialEq, Eq)]
enum PartitionKind {
    /// A partition that concretely exists
    Real,
    /// A partition whose creation has not yet been committed
    Virtual,
    /// A partition whose deletion has not yet been committed
    Hidden,
}

impl Partition {
    pub fn name(&self) -> &str {
        self.name.1.last().unwrap_or(&self.name.0).as_ref()
    }

    pub fn bounds(&self) -> &RangeInclusive<i64> {
        self.bounds.1.last().unwrap_or(&self.bounds.0)
    }

    pub fn fs(&self) -> Option<FileSystem> {
        self.fs.1.last().copied().unwrap_or(self.fs.0)
    }

    pub fn mounted(&self) -> bool {
        self.mount_point.is_some()
    }

    pub fn size(&self) -> Byte {
        let bounds = self.bounds();
        Byte::from_u64((bounds.end() - bounds.start()) as u64 * self.sector_size)
    }

    fn undo_all_changes(&mut self) {
        self.name.1.clear();
        self.bounds.1.clear();
        self.fs.1.clear();
    }

    fn from_libparted(
        value: libparted::Partition,
        sector_size: u64,
        mount_info: Option<&MountInfo>,
    ) -> Self {
        let path = value.get_path().map(Arc::from);
        Self {
            used: path.is_some(),
            path,
            mount_point: mount_info.map(|m| Arc::from(m.dest.as_ref())),
            kind: PartitionKind::Real,
            name: (value.name().unwrap_or_default().into(), Vec::new()),
            bounds: (value.geom_start()..=value.geom_end(), Vec::new()),
            fs: (
                #[allow(clippy::unwrap_used, reason = "statically impossible")]
                value.fs_type_name().map(|name| name.parse().unwrap()),
                Vec::new(),
            ),
            sector_size,
        }
    }

    fn new(
        name: Arc<str>,
        bounds: RangeInclusive<i64>,
        fs: Option<FileSystem>,
        used: bool,
        sector_size: u64,
    ) -> Self {
        Self {
            path: None,
            mount_point: None,
            used,
            kind: PartitionKind::Virtual,
            name: (name, Vec::new()),
            bounds: (bounds, Vec::new()),
            fs: (fs, Vec::new()),
            sector_size,
        }
    }
}
