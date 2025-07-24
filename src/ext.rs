use std::path::PathBuf;

use byte_unit::Byte;
use libparted::Device;

pub struct DeviceInfo {
    pub model: String,
    pub path: PathBuf,
    pub length: Byte,
}

impl From<&Device<'_>> for DeviceInfo {
    fn from(value: &Device<'_>) -> Self {
        Self {
            model: value.model().to_string(),
            path: value.path().to_owned(),
            length: Byte::from_u64(value.length() * value.sector_size()),
        }
    }
}

pub struct PartitionInfo {
    pub path: Option<PathBuf>,
    pub fs_type: Option<String>,
    pub length: Byte,
}
