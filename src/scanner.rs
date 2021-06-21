use crate::file::{FileContent, FileSet};
use crate::metadata::Metadata;
use std::cell::RefCell;
use std::cmp;
use std::collections::btree_map::Entry as BTreeEntry;
use std::collections::hash_map::Entry as HashEntry;
use std::collections::BTreeMap;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt::Debug;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RunMode {
    /// Merges paths in memory, but not on disk. Gives realistic UI output.
    DryRun,
    /// Like dry run, but completely skips deduping, with no UI for dupes.
    DryRunNoMerging,
    Hardlink,
}

#[derive(Debug)]
pub struct Settings {
    /// Ignore files smaller than a filesystem block.
    /// Deduping of such files is unlikely to save space.
    pub ignore_small: bool,
    pub run_mode: RunMode,

    // If 1, go to flush. If > 1, abort immediately.
    pub break_on: Option<&'static AtomicU32>,
}

impl Settings {
    pub fn breaks(&self) -> u32 {
        if let Some(break_on) = self.break_on {
            break_on.load(Ordering::SeqCst)
        } else {
            0
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "json", derive(serde_derive::Serialize))]
pub struct Stats {
    pub added: usize,
    pub skipped: usize,
    pub dupes: usize,
    pub bytes_deduplicated: usize,
    pub hardlinks: usize,
    pub bytes_saved_by_hardlinks: usize,
}

pub trait ScanListener: Debug {
    fn file_scanned(&mut self, path: &Path, stats: &Stats);
    fn scan_over(&self, scanner: &Scanner, stats: &Stats, scan_duration: Duration);
    fn hardlinked(&mut self, src: &Path, dst: &Path);
    fn duplicate_found(&mut self, src: &Path, dst: &Path);
}

#[derive(Debug)]
struct SilentListener;
impl ScanListener for SilentListener {
    fn file_scanned(&mut self, _: &Path, _: &Stats) {}

    fn scan_over(&self, _: &Scanner, _: &Stats, _: Duration) {}

    fn hardlinked(&mut self, _: &Path, _: &Path) {}

    fn duplicate_found(&mut self, _: &Path, _: &Path) {}
}

type RcFileSet = Rc<RefCell<FileSet>>;

#[derive(Debug)]
pub struct Scanner {
    /// All hardlinks of the same inode have to be treated as the same file
    by_inode: HashMap<(u64, u64), RcFileSet>,
    /// See Hasher for explanation
    by_content: BTreeMap<FileContent, Vec<RcFileSet>>,
    /// Directories left to scan. Sorted by inode number.
    /// I'm assuming scanning in this order is faster, since inode is related to file's age,
    /// which is related to its physical position on disk, which makes the scan more sequential.
    to_scan: BinaryHeap<(u64, PathBuf)>,

    scan_listener: Box<dyn ScanListener>,
    stats: Stats,
    exclude: HashSet<OsString>,
    pub settings: Settings,

    deferred_count: usize,
    next_deferred_count: usize,
}

impl Scanner {
    pub fn new() -> Self {
        Scanner {
            settings: Settings {
                ignore_small: true,
                run_mode: RunMode::Hardlink,
                break_on: None,
            },
            by_inode: HashMap::new(),
            by_content: BTreeMap::new(),
            to_scan: BinaryHeap::new(),
            scan_listener: Box::new(SilentListener),
            stats: Stats::default(),
            exclude: HashSet::new(),
            deferred_count: 0,
            next_deferred_count: 4096,
        }
    }

    pub fn exclude(&mut self, exclude: Vec<String>) {
        self.exclude = exclude.into_iter().map(From::from).collect();
    }

    /// Set the scan listener. Caution: This overrides previously set listeners!
    /// Use a multiplexing listener if multiple listeners are required.
    pub fn set_listener(&mut self, listener: Box<dyn ScanListener>) {
        self.scan_listener = listener;
    }

    /// Scan any file or directory for dupes.
    /// Dedupe is done within the path as well as against all previously added paths.
    pub fn scan(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        self.enqueue(path)?;
        self.flush()?;
        Ok(())
    }

    pub fn enqueue(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = fs::canonicalize(path)?;
        let metadata = fs::symlink_metadata(&path)?;
        self.add(path, &metadata)?;
        Ok(())
    }

    /// Drains the queue of directories to scan
    pub fn flush(&mut self) -> io::Result<()> {
        let start_time = Instant::now();
        while let Some((_, path)) = self.to_scan.pop() {
            if let Err(err) = self.scan_dir(&path) {
                eprintln!("Error scanning {}: {}", path.display(), err);
                self.stats.skipped += 1;
            }
            if self.settings.breaks() > 0 {
                eprintln!("Stopping scan");
                break;
            }
        }
        self.flush_deferred();
        let scan_duration = Instant::now().duration_since(start_time);
        self.scan_listener.scan_over(self, &self.stats, scan_duration);
        Ok(())
    }

    fn scan_dir(&mut self, path: &Path) -> io::Result<()> {
        // Errors are ignored here, since it's super common to find permission denied and unreadable symlinks,
        // and it'd be annoying if that aborted the whole operation.
        // FIXME: store the errors somehow to report them in a controlled manner
        for entry in fs::read_dir(path)?.filter_map(|p| p.ok()) {
            if self.settings.breaks() > 0 {
                break;
            }

            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if self.exclude.contains(file_name) {
                    self.stats.skipped += 1;
                    continue;
                }
            }
            if let Err(err) = self.add(path, &entry.metadata()?) {
                eprintln!("{}: {}", entry.path().display(), err);
            }
        }
        Ok(())
    }

    fn add(&mut self, path: PathBuf, metadata: &fs::Metadata) -> io::Result<()> {
        self.scan_listener.file_scanned(&path, &self.stats);

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

        // APFS reports 4*MB* block size
        let small_size = cmp::min(16 * 1024, metadata.blksize());
        if metadata.size() == 0 || (self.settings.ignore_small && metadata.size() < small_size) {
            self.stats.skipped += 1;
            return Ok(());
        }
        self.stats.added += 1;

        if let Some(fileset) = self.new_fileset(&path, metadata) {
            self.dedupe_by_content(fileset, path, metadata)?;
        } else {
            self.stats.hardlinks += 1;
            self.stats.bytes_saved_by_hardlinks += metadata.size() as usize;
        }
        Ok(())
    }

    /// Creates a new fileset if it's a new file.
    /// Returns None if it's a hardlink of a file already seen.
    fn new_fileset(&mut self, path: &Path, metadata: &fs::Metadata) -> Option<RcFileSet> {
        let path = path.to_path_buf();
        let device_inode = (metadata.dev(), metadata.ino());

        match self.by_inode.entry(device_inode) {
            HashEntry::Vacant(e) => {
                let fileset = Rc::new(RefCell::new(FileSet::new(path, metadata.nlink())));
                e.insert(Rc::clone(&fileset)); // clone just bumps a refcount here
                Some(fileset)
            },
            HashEntry::Occupied(mut e) => {
                // This case may require a deferred deduping later,
                // if the new link belongs to an old fileset that has already been deduped.
                let mut t = e.get_mut().borrow_mut();
                t.push(path);
                None
            },
        }
    }

    /// Here's where all the magic happens
    fn dedupe_by_content(&mut self, fileset: RcFileSet, path: PathBuf, metadata: &fs::Metadata) -> io::Result<()> {
        let mut deferred = false;
        match self.by_content.entry(FileContent::new(path, Metadata::new(metadata))) {
            BTreeEntry::Vacant(e) => {
                // Seems unique so far
                e.insert(vec![fileset]);
            },
            BTreeEntry::Occupied(mut e) => {
                // Found a dupe!
                self.stats.dupes += 1;
                self.stats.bytes_deduplicated += metadata.size() as usize;
                let filesets = e.get_mut();
                filesets.push(fileset);
                // Deduping can either be done immediately or later. Immediate is more cache-friendly and interactive,
                // but for files that already have hardlinks it can cause unnecessary re-linking. So if there are
                // hardlinks in the set, wait until the end to dedupe when all hardlinks are known.
                if filesets.iter().all(|set| set.borrow().links() == 1) {
                    Self::dedupe(filesets, self.settings.run_mode, &mut *self.scan_listener)?;
                } else {
                    deferred = true;
                }
            },
        }

        // Periodically flush deferred files to avoid building a huge queue
        // (the growing limit is a compromise between responsiveness
        // and potential to hit a pathological case of hardlinking with wrong hardlink groups)
        if deferred {
            self.deferred_count += 1;
            if self.deferred_count >= self.next_deferred_count {
                self.next_deferred_count *= 2;
                self.deferred_count = 0;
                self.flush_deferred();
            }
        }
        Ok(())
    }

    fn flush_deferred(&mut self) {
        for filesets in self.by_content.values_mut() {
            if self.settings.breaks() > 1 {
                eprintln!("Aborting");
                break;
            }
            if let Err(err) = Self::dedupe(filesets, self.settings.run_mode, &mut *self.scan_listener) {
                eprintln!("{}", err);
            }
        }
    }

    fn dedupe(filesets: &mut Vec<RcFileSet>, run_mode: RunMode, scan_listener: &mut dyn ScanListener) -> io::Result<()> {
        if run_mode == RunMode::DryRunNoMerging {
            return Ok(());
        }

        // Find file with the largest number of hardlinks, since it's less work to merge a small group into a large group
        let mut largest_idx = 0;
        let mut largest_links = 0;
        let mut nonempty_filesets = 0;
        for (idx, fileset) in filesets.iter().enumerate() {
            let fileset = fileset.borrow();
            if !fileset.paths.is_empty() {
                // Only actual paths we can merge matter here
                nonempty_filesets += 1;
            }
            let links = fileset.links();
            if links > largest_links {
                largest_idx = idx;
                largest_links = links;
            }
        }

        if nonempty_filesets == 0 {
            return Ok(()); // Already merged
        }

        // The set is still going to be in use! So everything has to be updated to make sense for the next call
        let merged_paths = &mut { filesets[largest_idx].borrow_mut() }.paths;
        let source_path = merged_paths[0].clone();
        for (i, set) in filesets.iter().enumerate() {
            // We don't want to merge the set with itself
            if i == largest_idx {
                continue;
            }

            let paths = &mut set.borrow_mut().paths;
            // dest_path will be "lost" on error, but that's fine, since we don't want to dedupe it if it causes errors
            for dest_path in paths.drain(..) {
                assert_ne!(&source_path, &dest_path);
                debug_assert_ne!(fs::symlink_metadata(&source_path)?.ino(), fs::symlink_metadata(&dest_path)?.ino());

                if run_mode == RunMode::DryRun {
                    scan_listener.duplicate_found(&dest_path, &source_path);
                    merged_paths.push(dest_path);
                    continue;
                }

                let temp_path = dest_path.with_file_name(".tmp-dupe-e1iIQcBFn5pC4MUSm-xkcd-221");
                debug_assert!(!temp_path.exists());
                debug_assert!(source_path.exists());
                debug_assert!(dest_path.exists());

                // In posix link guarantees not to overwrite, and mv guarantes to move atomically
                // so this two-step replacement is pretty robust
                if let Err(err) = fs::hard_link(&source_path, &temp_path) {
                    eprintln!("unable to hardlink {} {} due to {}", source_path.display(), temp_path.display(), err);
                    let _ = fs::remove_file(temp_path);
                    return Err(err);
                }
                if let Err(err) = fs::rename(&temp_path, &dest_path) {
                    eprintln!("unable to rename {} {} due to {}", temp_path.display(), dest_path.display(), err);
                    let _ = fs::remove_file(temp_path);
                    return Err(err);
                }
                debug_assert!(!temp_path.exists());
                debug_assert!(source_path.exists());
                debug_assert!(dest_path.exists());
                scan_listener.hardlinked(&dest_path, &source_path);
                merged_paths.push(dest_path);
            }
        }
        Ok(())
    }

    pub fn dupes(&self) -> Vec<Vec<FileSet>> {
        self.by_content.iter().map(|(_,filesets)|{
            filesets.iter().map(|d|{
                let tmp = d.borrow();
                (*tmp).clone()
            }).collect()
        }).collect()
    }
}

