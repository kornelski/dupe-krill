use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::Path;

#[derive(Copy, Clone, Hash, Ord, PartialOrd, PartialEq, Eq, Debug, Default)]
pub struct Metadata {
    pub dev: u64,
    pub size: u64,
}

impl Metadata {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let m = fs::symlink_metadata(path)?;
        Ok(Self::new(&m))
    }

    pub fn new(m: &fs::Metadata) -> Self {
        Metadata {
            dev: get_device_id(m),
            size: m.len(),
        }
    }
}

#[cfg(unix)]
fn get_device_id(m: &fs::Metadata) -> u64 {
    m.dev()
}

#[cfg(windows)]
fn get_device_id(m: &fs::Metadata) -> u64 {
    // On Windows, it's possible to use the volume serial number as a device identifier
    // For now, use a constant since Windows doesn't have a direct equivalent
    // This means hardlinking across different drives won't work, but that's expected
    use std::os::windows::fs::MetadataExt;
    m.volume_serial_number().unwrap_or(0) as u64
}
