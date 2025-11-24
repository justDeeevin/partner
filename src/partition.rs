use byte_unit::Byte;
use proc_mounts::MountInfo;
use std::{fmt::Debug, ops::RangeInclusive, path::Path, sync::Arc};
use strum::{Display, EnumString};

pub struct Partition {
    pub path: Option<Arc<Path>>,
    // TODO
    // pub occupied: Byte,
    pub mount_point: Option<Arc<Path>>,
    pub(crate) kind: PartitionKind,
    pub(crate) name: (Arc<str>, Vec<Arc<str>>),
    pub(crate) bounds: (RangeInclusive<i64>, Vec<RangeInclusive<i64>>),
    pub(crate) fs: (Option<FileSystem>, Vec<Option<FileSystem>>),
    sector_size: u64,
}

impl Debug for Partition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Partition")
            .field("path", &self.path)
            .field("mount_point", &self.mount_point)
            .field("name", &self.name())
            .field("bounds", self.bounds())
            .field("fs", &self.fs())
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PartitionKind {
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

    /// The bounds of the partition **in sectors**.
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

    pub fn used(&self) -> bool {
        self.fs().is_some() || self.path.is_some()
    }

    pub(crate) fn undo_all_changes(&mut self) {
        self.name.1.clear();
        self.bounds.1.clear();
        self.fs.1.clear();
    }

    pub(crate) fn from_libparted(
        value: libparted::Partition,
        sector_size: u64,
        mount_info: Option<&MountInfo>,
    ) -> Self {
        let path = value.get_path().map(Arc::from);
        Self {
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

    pub(crate) fn new(
        name: Arc<str>,
        bounds: RangeInclusive<i64>,
        fs: Option<FileSystem>,
        sector_size: u64,
    ) -> Self {
        Self {
            path: None,
            mount_point: None,
            kind: PartitionKind::Virtual,
            name: (name, Vec::new()),
            bounds: (bounds, Vec::new()),
            fs: (fs, Vec::new()),
            sector_size,
        }
    }
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
