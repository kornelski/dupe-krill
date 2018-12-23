use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[derive(Copy, Clone, Hash, Ord, PartialOrd, PartialEq, Eq, Debug, Default)]
pub struct Metadata {
    pub dev: u64,
    pub size: u64,
}

impl Metadata {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let m = fs::symlink_metadata(&path)?;
        Ok(Self::new(&m))
    }

    pub fn new(m: &fs::Metadata) -> Self {
        Metadata {
            dev: m.dev(),
            size: m.size(),
        }
    }
}
