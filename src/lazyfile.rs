use std::fs;
use std::io;
use std::path::Path;

/// Open the file only if necessary.
/// The file will be closed automatically when this object goes out of scope.
pub struct LazyFile<'a> {
    path: &'a Path,
    file: Option<fs::File>,
}

impl<'a> LazyFile<'a> {
    pub fn new(path: &'a Path) -> Self {
        LazyFile {
            path, file: None,
        }
    }

    /// Open the file (or reuse already-opened handle)
    pub fn fd(&mut self) -> Result<&mut fs::File, io::Error> {
        if let Some(ref mut fd) = self.file {
            Ok(fd)
        } else {
            self.file = Some(fs::File::open(self.path)?);
            if let Some(ref mut fd) = self.file {
                Ok(fd)
            } else {
                unreachable!();
            }
        }
    }
}
