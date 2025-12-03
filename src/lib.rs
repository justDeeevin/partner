#![deny(clippy::unwrap_used)]

//! A Linux disk partitioning library.
//!
//! This library uses [libparted] under the hood, and is intended to be simpler and more
//! convenient, with built-in support for undoing changes and owned types for partitions and disks.

mod partition;

use either::Either;
pub use partition::*;

use byte_unit::Byte;
use libparted::Geometry;
use proc_mounts::MountInfo;
use std::{
    collections::HashMap,
    fmt::Debug,
    ops::{Bound, RangeBounds, RangeInclusive},
    path::{Path, PathBuf},
    sync::Arc,
};

type RawDevice<'a> = libparted::Device<'a>;

/// A storage device.
///
/// Changes are not written to disk until [`commit`](Device::commit) is called.
pub struct Device<'a> {
    model: Arc<str>,
    path: Arc<Path>,
    partitions: Vec<Partition>,
    changes: Vec<InnerChange>,
    raw: RawDevice<'a>,
}

impl Debug for Device<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("model", &self.model)
            .field("path", &self.path)
            .field("size", &self.size())
            .field("partitions", &self.partitions().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("given bounds overlap with existing partition №{0}")]
    OverlapsExisting(usize),
    #[error("given bounds are out of device bounds")]
    OutOfBounds,
}

impl<'a> Device<'a> {
    fn get_mounts() -> std::io::Result<HashMap<PathBuf, MountInfo>> {
        Ok(proc_mounts::MountIter::new()?
            .flatten()
            .map(|m| (m.source.clone(), m))
            .collect())
    }

    /// Open a device from the given block device path.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::from_libparted(RawDevice::new(path)?, &Self::get_mounts()?)
    }

    /// Get all devices on the system.
    ///
    /// This isn't necessarily all of the available devices (for instance, this ignores loopback
    /// devices). [`open`](Device::open) can be used to open a specific device if you're looking
    /// for one not returned by this.
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
        let partitions = libparted::Disk::new(&mut value)?
            .parts()
            .filter_map(|p| {
                let mount = mounts.get(p.get_path()?);
                Some(Partition::from_libparted(p, sector_size, mount))
            })
            .collect::<Vec<_>>();
        Ok(Self {
            model: value.model().into(),
            path: value.path().into(),
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
        Byte::from_u64(self.raw.length() * self.raw.sector_size())
    }

    pub fn partitions(&self) -> impl Iterator<Item = &Partition> {
        self.partitions
            .iter()
            .filter(|p| p.kind != PartitionKind::Hidden)
    }

    /// Get partitions interspersed with ranges of unused sectors.
    ///
    /// [`partitions`](Device::partitions) produces only partitions, leaving the caller to infer
    /// unused sectors based on gaps in partition bounds. This function does that work for you.
    #[allow(clippy::unwrap_used, reason = "panic statically impossible")]
    pub fn partitions_with_empty(&self) -> Vec<Either<&Partition, RangeInclusive<i64>>> {
        fn as_left<T, U>(either: &Either<T, U>) -> Option<&T> {
            match either {
                Either::Left(l) => Some(l),
                Either::Right(_) => None,
            }
        }

        let mut partitions = self.partitions().map(Either::Left).collect::<Vec<_>>();
        if !partitions.is_empty() {
            let mut i = 0;
            if *as_left(&partitions[0]).unwrap().bounds().start() > 1 {
                partitions.insert(
                    0,
                    Either::Right(1..=as_left(&partitions[0]).unwrap().bounds().start() - 1),
                );
                i += 1;
            }
            while i < partitions.len() - 1 {
                let left = *as_left(&partitions[i]).unwrap().bounds().end();
                let right = *as_left(&partitions[i + 1]).unwrap().bounds().start();
                assert!(right > left, "overlapping partitions");
                if right - left > 1 {
                    partitions.insert(i + 1, Either::Right(left + 1..=right - 1));
                    i += 1;
                }

                i += 1;
            }
            let end = *partitions.last().and_then(as_left).unwrap().bounds().end();
            if Byte::from_u64(end as u64 * self.sector_size()) < self.size() {
                partitions.push(Either::Right(
                    end..=(self.size().as_u64() / self.sector_size()) as i64,
                ));
            }
        }

        partitions
    }

    pub fn sector_size(&self) -> u64 {
        self.raw.sector_size()
    }

    fn partitions_enum(&self) -> impl Iterator<Item = (usize, &Partition)> {
        self.partitions
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind != PartitionKind::Hidden)
    }

    /// Get the number of pending changes.
    pub fn n_changes(&self) -> usize {
        self.changes.len()
    }

    pub fn change_partition_name(&mut self, partition: usize, new: Arc<str>) {
        self.partitions[partition].name.1.push(new.clone());
        self.changes.push(InnerChange::Name { partition, new });
    }

    /// Create a new partition with the given name, (optionally) filesystem, and bounds **in
    /// sectors**.
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

        let index = {
            let mut iter = self.partitions_enum().peekable();
            let mut out = 0;

            while let Some((i, p)) = iter.next() {
                if p.bounds().end() < bounds.start()
                    && iter
                        .peek()
                        .is_some_and(|(_, p)| p.bounds().start() > bounds.end())
                {
                    out = i;
                    break;
                } else if p.bounds().end() <= bounds.start() {
                    return Err(Error::OverlapsExisting(i));
                } else if iter
                    .peek()
                    .is_some_and(|(_, p)| p.bounds().start() <= bounds.end())
                {
                    return Err(Error::OverlapsExisting(i + 1));
                }
            }

            out
        };

        self.partitions.insert(
            index,
            Partition::new(name.clone(), bounds.clone(), fs, self.raw.sector_size()),
        );

        self.changes.push(InnerChange::NewPartition {
            name,
            fs,
            bounds,
            index,
        });

        Ok(())
    }

    /// Remove the partition at the given index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    pub fn remove_partition(&mut self, index: usize) {
        let index = self
            .partitions_enum()
            .nth(index)
            .expect("partition index out of bounds")
            .0;
        let removed = if self.partitions[index].kind == PartitionKind::Virtual {
            Some(self.partitions.remove(index))
        } else {
            self.partitions[index].kind = PartitionKind::Hidden;
            None
        };

        self.changes
            .push(InnerChange::RemovePartition { index, removed });
    }

    /// Change the bounds of the partition at the given index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    pub fn resize_partition(
        &mut self,
        index: usize,
        new_bounds: impl RangeBounds<i64>,
    ) -> Result<(), Error> {
        let bounds = match new_bounds.start_bound() {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => b + 1,
            Bound::Unbounded => 0,
        }..=match new_bounds.end_bound() {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => b - 1,
            Bound::Unbounded => self.raw.length() as i64,
        };

        let index = self
            .partitions_enum()
            .nth(index)
            .expect("partition index out of bounds")
            .0;

        if *bounds.start() < 0 || *bounds.end() > self.raw.length() as i64 {
            Err(Error::OutOfBounds)
        } else if index != 0 && self.partitions[index - 1].bounds().end() > bounds.start() {
            Err(Error::OverlapsExisting(index - 1))
        } else if self.partitions[index + 1].bounds().start() < bounds.end() {
            Err(Error::OverlapsExisting(index + 1))
        } else {
            self.partitions[index].bounds.1.push(bounds.clone());
            self.changes
                .push(InnerChange::ResizePartition { index, bounds });
            Ok(())
        }
    }

    #[allow(clippy::unwrap_used, reason = "a failure here would be a logic bug")]
    fn get_public_index(&self, index: usize) -> usize {
        self.partitions_enum().position(|p| p.0 == index).unwrap()
    }

    /// Undo the last change.
    pub fn undo_change(&mut self) -> Option<Change> {
        match self.changes.pop() {
            Some(InnerChange::Name { partition, new }) => {
                self.partitions[partition].name.1.pop();
                Some(Change::Name { partition, new })
            }
            Some(InnerChange::NewPartition { index, .. }) => {
                assert!(
                    self.partitions[index].kind == PartitionKind::Virtual,
                    "undo tried to remove a real partition"
                );
                self.remove_partition(index);
                Some(Change::RemovePartition {
                    index: self.get_public_index(index),
                })
            }
            #[allow(clippy::unwrap_used, reason = "a failure here would be a logic bug")]
            Some(InnerChange::RemovePartition { index, removed }) => {
                if let Some(removed) = removed {
                    self.partitions.insert(index, removed);
                } else {
                    assert!(
                        self.partitions[index].kind == PartitionKind::Hidden,
                        "undo tried to set a virtual partition to real"
                    );
                    self.partitions[index].kind = PartitionKind::Real;
                }
                Some(Change::RemovePartition {
                    index: self.get_public_index(index),
                })
            }
            Some(InnerChange::ResizePartition { index, bounds }) => {
                self.partitions[index].bounds.1.pop();
                Some(Change::ResizePartition {
                    index: self.get_public_index(index),
                    bounds,
                })
            }
            None => None,
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

    /// Commit all changes to the device.
    ///
    /// This is blocking and will likely take a while.
    pub fn commit(&mut self) -> std::io::Result<()> {
        let mut disk = libparted::Disk::new(&mut self.raw)?;

        for change in self.changes.drain(..) {
            change.apply(&mut disk)?;
        }

        disk.commit()
    }
}

enum InnerChange {
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
    RemovePartition {
        index: usize,
        removed: Option<Partition>,
    },
    ResizePartition {
        index: usize,
        bounds: RangeInclusive<i64>,
    },
}

/// A change to a device returned by [`Device::undo_change`].
pub enum Change {
    Name {
        partition: usize,
        new: Arc<str>,
    },
    NewPartition {
        name: Arc<str>,
        fs: Option<FileSystem>,
        bounds: RangeInclusive<i64>,
    },
    RemovePartition {
        index: usize,
    },
    ResizePartition {
        index: usize,
        bounds: RangeInclusive<i64>,
    },
}

impl InnerChange {
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
            Self::RemovePartition { index, .. } => {
                disk.remove_partition_by_number(index as u32 + 1)
            }
            #[allow(
                clippy::unwrap_used,
                reason = "a panic here would be an internal logic bug"
            )]
            Self::ResizePartition { index, bounds } => disk
                .get_partition(index as u32)
                .unwrap()
                .get_geom()
                .open_fs()
                .unwrap()
                .resize(
                    &Geometry::new(
                        &unsafe { disk.get_device() },
                        *bounds.start(),
                        bounds.end() - bounds.start(),
                    )?,
                    None,
                ),
        }
    }
}
