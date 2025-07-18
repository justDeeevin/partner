use byte_unit::Byte;
use libparted::Device;

pub trait DeviceExt {
    fn size(&self) -> Byte;
}

impl DeviceExt for Device<'_> {
    fn size(&self) -> Byte {
        Byte::from_u64(self.length() * self.sector_size())
    }
}
