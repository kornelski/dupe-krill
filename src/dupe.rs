use std::fs;
use std::io;
use file::{FileContent, FileSet};
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::BinaryHeap;
use metadata::Metadata;
use std::rc::Rc;
use std::sync::Mutex;
use std::os::unix::fs::MetadataExt;
use std::collections::hash_map::Entry as HashEntry;
use std::collections::btree_map::Entry as BTreeEntry;
use std::time::Instant;

#[derive(Debug)]
pub struct Settings {
    // Ignore files smaller than a filesystem block.
    // Deduping of such files is unlikely to save space.
    pub ignore_small: bool
}

#[derive(Debug,Default)]
struct Stats {
    added: usize,
    skipped: usize,
    dupes: usize,
    hardlinks: usize,
}

#[derive(Debug)]
struct UITiming {
    // Time in seconds, used to throttle console output
    next_update: u64,
    start_time: Instant,
}

#[derive(Debug)]
pub struct Scanner {
    /// All hardlinks of the same inode have to be treated as the same file
    by_inode: HashMap<(u64, u64), Rc<Mutex<FileSet>>>,
    /// See Hasher for explanation
    by_content: BTreeMap<FileContent, Vec<Rc<Mutex<FileSet>>>>,
    /// Directories left to scan. Sorted by inode number.
    /// I'm assuming scanning in this order is faster, since inode is related to file's age,
    /// which is related to its physical position on disk, which makes the scan more sequential.
    to_scan: BinaryHeap<(u64, PathBuf)>,

    ui_timing: UITiming,
    stats: Stats,
    pub settings: Settings,
}

impl Scanner {
    pub fn new() -> Self {
        Scanner {
            settings: Settings {
                ignore_small: true,
            },
            by_inode: HashMap::new(),
            by_content: BTreeMap::new(),
            to_scan: BinaryHeap::new(),
            ui_timing: UITiming {
                next_update: 0,
                start_time: Instant::now(),
            },
            stats: Stats::default(),
        }
    }

    /// Scan any file or directory for dupes.
    /// Dedupe is done within the path as well as against all previously added paths.
    pub fn scan<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.enqueue(path)?;
        self.flush()?;
        Ok(())
    }

    pub fn enqueue<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let path = fs::canonicalize(path)?;
        let metadata = fs::symlink_metadata(&path)?;
        self.add(path, metadata)?;
        Ok(())
    }

    /// Drains the queue of directories to scan
    pub fn flush(&mut self) -> io::Result<()> {
        while let Some((_, path)) = self.to_scan.pop() {
            self.scan_dir(path)?;
        }
        Ok(())
    }

    fn scan_dir(&mut self, path: PathBuf) -> io::Result<()> {
        /// Errors are ignored here, since it's super common to find permission denied and unreadable symlinks,
        /// and it'd be annoying if that aborted the whole operation.
        // FIXME: store the errors somehow to report them in a controlled manner
        for entry in fs::read_dir(path)?.filter_map(|p|p.ok()) {
            let path = entry.path();
            self.add(path, entry.metadata()?).unwrap_or_else(|e| println!("{:?}", e));
        }
        Ok(())
    }

    fn update_ui(&mut self, path: &PathBuf) {
        let elapsed = self.ui_timing.start_time.elapsed().as_secs();
        if elapsed > self.ui_timing.next_update {
            self.ui_timing.next_update = elapsed+1;
            println!("{}+{} dupes. {}+{} files scanned. {} dirs left. {}/…",
                self.stats.dupes, self.stats.hardlinks, self.stats.added, self.stats.skipped, self.to_scan.len(),
                path.parent().unwrap_or(path).display());
        }
    }

    fn add(&mut self, path: PathBuf, metadata: fs::Metadata) -> io::Result<()> {
        self.update_ui(&path);

        let ty = metadata.file_type();
        if ty.is_dir() {
            // Inode is truncated to group scanning of roughly close inodes together,
            // But still preserve some directory traversal order.
            // Negation to scan from the highest (assuming latest) first.
            let order_key = !(metadata.ino() >> 8);
            self.to_scan.push((order_key, path));
            return Ok(());
        } else if ty.is_symlink() {
            // Support for traversing symlinks would require preventing loops
            self.stats.skipped += 1;
            return Ok(());
        } else if !ty.is_file() {
            // Deduping /dev/ would be funny
            self.stats.skipped += 1;
            return Ok(());
        }

        if self.settings.ignore_small && metadata.size() < metadata.blksize() {
            self.stats.skipped += 1;
            return Ok(());
        }

        self.stats.added += 1;

        let path_hardlinks = metadata.nlink();
        let m = (metadata.dev(), metadata.ino());

        // That's handling hardlinks
        let fileset = match self.by_inode.entry(m) {
            HashEntry::Vacant(e) => {
                let fileset = Rc::new(Mutex::new(FileSet::new(path.clone(), path_hardlinks)));
                e.insert(fileset.clone()); // clone just bumps a refcount here
                fileset
            },
            HashEntry::Occupied(mut e) => {
                self.stats.hardlinks += 1;
                let mut t = e.get_mut().lock().unwrap();
                t.push(path, path_hardlinks);
                return Ok(());
            }
        };

        // Here's where all the magic happens
        match self.by_content.entry(FileContent::new(path, Metadata::new(&metadata))) {
            BTreeEntry::Vacant(e) => {
                // Seems unique so far
                e.insert(vec![fileset]);
            },
            BTreeEntry::Occupied(mut e) => {
                // Found a dupe!
                self.stats.dupes += 1;
                let filesets = e.get_mut();
                filesets.push(fileset);
                Self::dedupe(filesets)?;
            },
        }
        Ok(())
    }

    fn dedupe(filesets: &mut Vec<Rc<Mutex<FileSet>>>) -> io::Result<()> {
        // Find file with the largest number of hardlinks, since it's less work to merge a small group into a large group
        let (largest_idx, merged_fileset) = filesets.iter().enumerate().max_by_key(|&(i,f)| (f.lock().unwrap().links(),!i)).expect("fileset can't be empty");

        // The set is still going to be in use! So everything has to be updated to make sense for the next call
        let merged_paths = &mut merged_fileset.lock().unwrap().paths;
        let source_path = merged_paths[0].clone();
        for (i, set) in filesets.iter().enumerate() {
            // We don't want to merge the set with itself
            if i == largest_idx {continue;}

            let paths = &mut set.lock().unwrap().paths;
            // dest_path will be "lost" on error, but that's fine, since we don't want to dedupe it if it causes errors
            for dest_path in paths.drain(..) {
                assert_ne!(&source_path, &dest_path);
                debug_assert_ne!(fs::symlink_metadata(&source_path)?.ino(), fs::symlink_metadata(&dest_path)?.ino());

                let temp_path = dest_path.with_file_name(".tmp-dupe-e1iIQcBFn5pC4MUSm-xkcd-221");
                debug_assert!(!temp_path.exists());
                debug_assert!(source_path.exists());
                debug_assert!(dest_path.exists());

                // In posix link guarantees not to overwrite, and mv guarantes to move atomically
                // so this two-step replacement is pretty robust
                if let Err(err) = fs::hard_link(&source_path, &temp_path) {
                    println!("unable to hardlink {} {} due to {:?}", source_path.display(), temp_path.display(), err);
                    fs::remove_file(temp_path).ok();
                    return Err(err);
                }
                if let Err(err) = fs::rename(&temp_path, &dest_path) {
                    println!("unable to rename {} {} due to {:?}", temp_path.display(), dest_path.display(), err);
                    fs::remove_file(temp_path).ok();
                    return Err(err);
                }
                debug_assert!(!temp_path.exists());
                debug_assert!(source_path.exists());
                debug_assert!(dest_path.exists());
                println!("Hardlinked {}", combined_paths(&dest_path, &source_path));
                merged_paths.push(dest_path);
            }
        }
        Ok(())
    }

    pub fn dupes(&self) -> Vec<FileSet> {
        self.by_inode.iter().map(|(_,d)|{
            let tmp = d.lock().unwrap();
            (*tmp).clone()
        }).collect()
    }
}

fn combined_paths(base: &Path, relativize: &Path) -> String {
    let base: Vec<_> = base.iter().collect();
    let relativize: Vec<_> = relativize.iter().collect();

    let mut out = String::with_capacity(80);
    let mut prefix_len = 0;
    for (comp,_) in base.iter().zip(relativize.iter()).take_while(|&(a,b)| a==b) {
        prefix_len += 1;
        let comp = comp.to_string_lossy();
        out += &comp;
        if comp != "/" {
            out.push('/');
        }
    }

    let suffix: Vec<_> = base.iter().skip(prefix_len).rev().zip(relativize.iter().skip(prefix_len).rev())
        .take_while(|&(a,b)| a==b).map(|(_,b)|b.to_string_lossy()).collect();

    let base_unique: Vec<_> = base[prefix_len..base.len()-suffix.len()].iter().map(|b|b.to_string_lossy()).collect();

    out.push('{');
    if base_unique.is_empty() {
        out.push('.');
    } else {
        out += &base_unique.join("/");
    }
    out += " => ";

    let rel_unique: Vec<_> = relativize[prefix_len..relativize.len()-suffix.len()].iter().map(|b|b.to_string_lossy()).collect();
    if rel_unique.is_empty() {
        out.push('.');
    } else {
        out += &rel_unique.join("/");
    }
    out.push('}');

    for comp in suffix.into_iter().rev() {
        out.push('/');
        out += &comp;
    }
    out
}

#[test]
fn scan() {
    let mut d = Scanner::new();
    d.scan("tests").unwrap();
}


#[test]
fn scan_hardlink() {
    use std::io::Write;
    use tempdir::TempDir;

    let dir = TempDir::new("hardlinktest2").unwrap();
    let a_path = dir.path().join("a");
    let b_path = dir.path().join("b");

    let mut a_fd = fs::File::create(&a_path).unwrap();
    a_fd.write_all(b"dupe").unwrap();
    drop(a_fd);

    fs::hard_link(&a_path, &b_path).unwrap();

    let mut d = Scanner::new();
    d.settings.ignore_small = false;
    d.scan(dir.path()).unwrap();
    let dupes = d.dupes();
    assert_eq!(dupes.len(), 1);
    assert_eq!(dupes[0].paths.len(), 2);
}

#[test]
fn combined_test() {
    let a: PathBuf = "foo/bar/baz/a.txt".into();
    let b: PathBuf = "foo/baz/quz/zzz/a.txt".into();
    let c: PathBuf = "foo/baz/quz/zzz/b.txt".into();
    let d: PathBuf = "b.txt".into();
    let e: PathBuf = "e.txt".into();
    let f: PathBuf = "/foo/bar/baz/a.txt".into();
    let g: PathBuf = "/foo/baz/quz/zzz/a.txt".into();
    let h: PathBuf = "/foo/b/quz/zzz/a.txt".into();

    assert_eq!(&combined_paths(&a,&b), "foo/{bar/baz => baz/quz/zzz}/a.txt");
    assert_eq!(&combined_paths(&c,&b), "foo/baz/quz/zzz/{b.txt => a.txt}");
    assert_eq!(&combined_paths(&c,&d), "{foo/baz/quz/zzz => .}/b.txt");
    assert_eq!(&combined_paths(&d,&c), "{. => foo/baz/quz/zzz}/b.txt");
    assert_eq!(&combined_paths(&d,&e), "{b.txt => e.txt}");
    assert_eq!(&combined_paths(&f,&g), "/foo/{bar/baz => baz/quz/zzz}/a.txt");
    assert_eq!(&combined_paths(&h,&g), "/foo/{b => baz}/quz/zzz/a.txt");
}
