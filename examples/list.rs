use partner::Device;

fn main() {
    let devices = Device::get_all().unwrap();
    if let Some(device) = std::env::args().nth(1) {
        let device = devices
            .into_iter()
            .find(|d| d.path() == &device)
            .unwrap_or_else(|| Device::open(device).unwrap());
        dbg!(device);
    } else {
        dbg!(devices);
    }
}
