use sha1::Sha1;
use std::cmp::{min, Ordering};
use std::io;
use std::io::{Read,Seek,SeekFrom};
use std::path::Path;
use lazyfile::LazyFile;

/// A hashed chunk of data of arbitrary size. Files are compared a bit by bit.
#[derive(Debug, PartialOrd, Eq, PartialEq, Ord)]
struct HashedRange {
    hash: [u8; 20],
    size: u64,
}

impl HashedRange {
    pub fn from_file(file: &mut LazyFile, start: u64, size: u64) -> Result<Self, io::Error> {
        let fd = file.fd()?;
        let mut data = vec![0; size as usize];
        fd.seek(SeekFrom::Start(start))?;
        fd.read_exact(&mut data)?;
        let mut sha1 = Sha1::new();
        // So the shattered PDFs don't dedupe
        sha1.update(b"ISpent$75KToCollideWithThisStringAndAllIGotWasADeletedFile");
        sha1.update(&data);

        Ok(HashedRange {
            hash: sha1.digest().bytes(),
            size: size,
        })
    }
}

#[derive(Debug)]
pub struct Hasher {
    ranges: Vec<HashedRange>,
}

/// Compares two files using hashes by hashing incrementally until the first difference is found
struct HashIter<'a> {
    pub index: usize,
    pub start_offset: u64,
    pub end_offset: u64,
    next_buffer_size: u64,
    a_file: LazyFile<'a>,
    b_file: LazyFile<'a>,
}

impl<'h> HashIter<'h> {
    pub fn new(size: u64, a_path: &'h Path, b_path: &'h Path) -> Self {
        HashIter {
            index: 0,
            start_offset: 0,
            end_offset: size,
            next_buffer_size: 4096,
            a_file: LazyFile::new(a_path),
            b_file: LazyFile::new(b_path),
        }
    }

    /// Compare (and compute if needed) the next two hashes
    pub fn next<'a,'b>(&mut self, a_hash: &'a mut Hasher, b_hash: &'b mut Hasher) -> Result<Option<(&'a HashedRange, &'b HashedRange)>, io::Error> {
        if self.start_offset >= self.end_offset {
            return Ok(None);
        }

        let i = self.index;
        let (a_none, b_none, size) = {
            let a = a_hash.ranges.get(i);
            let b = b_hash.ranges.get(i);

            // If there is an existing hashed chunk, the chunk size used for comparison must obviously be it.
            let size = a.map(|a|a.size).or(b.map(|b|b.size))
                .unwrap_or(min(self.end_offset - self.start_offset, self.next_buffer_size));
            (a.is_none(), b.is_none(), size)
        };

        // If any of the ranges is missing, compute it
        if a_none {
            a_hash.ranges.push(HashedRange::from_file(&mut self.a_file, self.start_offset, size)?);
        }
        if b_none {
            b_hash.ranges.push(HashedRange::from_file(&mut self.b_file, self.start_offset, size)?);
        }

        self.index += 1;
        self.start_offset += size;
        // The buffer size is a trade-off between finding a difference quickly
        // and reading files one by one without trashing.
        // Exponential increase is meant to be a compromise that allows finding
        // the difference in the first few KB, but grow quickly to read identical files faster.
        self.next_buffer_size = min(size * 8, 128*1024*1024);

        Ok(Some((&a_hash.ranges[i], &b_hash.ranges[i])))
    }
}

impl Hasher {
    pub fn new() -> Self {
        Hasher {
            ranges: Vec::new(),
        }
    }

    /// Incremental comparison reading files lazily
    pub fn compare(&mut self, other: &mut Hasher, size: u64, self_path: &Path, other_path: &Path) -> Result<Ordering,io::Error> {
        let mut iter = HashIter::new(size, self_path, other_path);

        while let Some((a,b)) = iter.next(self, other)? {
            let ord = a.cmp(b);
            if ord != Ordering::Equal {
                return Ok(ord);
            }
        }
        Ok(Ordering::Equal)
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;
    extern crate file;
    use super::*;

    #[test]
    fn range_sha() {
        let tmp = tempdir::TempDir::new("hashtest").expect("tmp");
        let path = &tmp.path().join("a");
        file::put_text(&path, "aaa\n").expect("write");
        let mut file = LazyFile::new(&path);
        let hashed = HashedRange::from_file(&mut file, 0, 4).expect("hash");

        assert_eq!(4, hashed.size);
        assert_eq!([199,31,32,178,46,189,89,221,26,72,162,140,182,69,43,154,40,195,32,163], hashed.hash);

        let hashed = HashedRange::from_file(&mut file, 1, 2).expect("hash2");
        assert_eq!(2, hashed.size);
    }
}
