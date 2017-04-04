extern crate tempdir;
extern crate duplicate_kriller;

use duplicate_kriller::*;
use std::io::Write;
use std::fs;
use tempdir::TempDir;

#[test]
fn scan() {
    let mut d = Scanner::new();
    d.scan("tests").unwrap();
}


#[test]
fn scan_hardlink() {

    let dir = TempDir::new("hardlinktest2").unwrap();
    let a_path = dir.path().join("a");
    let b_path = dir.path().join("b");

    let mut a_fd = fs::File::create(&a_path).unwrap();
    a_fd.write_all(b"dupe").unwrap();
    drop(a_fd);

    fs::hard_link(&a_path, &b_path).unwrap();

    let mut d = Scanner::new();
    d.settings.ignore_small = false;
    d.settings.dry_run = true;
    d.scan(dir.path()).unwrap();
    let dupes = d.dupes();
    assert_eq!(dupes.len(), 1);
    assert_eq!(dupes[0].paths.len(), 2);

    let mut d = Scanner::new();
    d.settings.ignore_small = false;
    d.scan(dir.path()).unwrap();
    let dupes = d.dupes();
    assert_eq!(dupes.len(), 1);
    assert_eq!(dupes[0].paths.len(), 2);
}
