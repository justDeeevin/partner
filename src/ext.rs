use std::{path::Path, sync::Arc};

use byte_unit::Byte;
use libparted::Device;

pub struct DeviceInfo {
    pub model: Arc<str>,
    pub path: Arc<Path>,
    pub length: Byte,
}

impl From<&Device<'_>> for DeviceInfo {
    fn from(value: &Device<'_>) -> Self {
        Self {
            model: value.model().into(),
            path: value.path().into(),
            length: Byte::from_u64(value.length() * value.sector_size()),
        }
    }
}

pub struct PartitionInfo {
    pub path: Option<Arc<Path>>,
    pub fs_type: Option<Arc<str>>,
    pub length: Byte,
    pub label: Option<Arc<str>>,
}
