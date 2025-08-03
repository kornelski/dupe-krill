use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
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
            size: get_size(m),
        }
    }
}

#[cfg(unix)]
fn get_device_id(m: &fs::Metadata) -> u64 {
    m.dev()
}

#[cfg(windows)]
fn get_device_id(_m: &fs::Metadata) -> u64 {
    // On Windows, we'll use a simple constant for device identification
    // This means hardlinking across different drives won't work properly,
    // but that's expected behavior and matches filesystem limitations
    // TODO: In the future, we could use Windows-specific APIs to get proper device IDs
    0
}

#[cfg(unix)]
fn get_size(m: &fs::Metadata) -> u64 {
    m.size()
}

#[cfg(windows)]
fn get_size(m: &fs::Metadata) -> u64 {
    // Windows polyfill: round up to the next 4KB block to account for block overhead
    let len = m.len();
    const BLOCK_SIZE: u64 = 4096;
    ((len + BLOCK_SIZE - 1) / BLOCK_SIZE) * BLOCK_SIZE
}
