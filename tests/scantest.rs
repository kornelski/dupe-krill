extern crate tempdir;
extern crate file;
extern crate duplicate_kriller;

use duplicate_kriller::*;
use std::fs;
use tempdir::TempDir;

#[test]
fn scan() {
    let mut d = Scanner::new();
    d.scan("tests").unwrap();
}


#[test]
fn test_exclude() {
    let dir = TempDir::new("excludetest").unwrap();
    let a_path = dir.path().join("a");
    let b_path = dir.path().join("b");
    file::put(a_path, "foo").unwrap();
    file::put(b_path, "foo").unwrap();

    let mut d = Scanner::new();
    d.settings.ignore_small = false;
    d.settings.run_mode = RunMode::DryRunNoMerging;
    d.exclude(vec!["b".to_string()]);

    d.scan(dir.path()).unwrap();
    let dupes = d.dupes();
    assert_eq!(dupes.len(), 1);
    assert_eq!(dupes[0].len(), 1);
    assert_eq!(dupes[0][0].paths.len(), 1);
}

#[test]
fn scan_hardlink() {

    let dir = TempDir::new("hardlinktest2").unwrap();
    let a_path = dir.path().join("a");
    let b_path = dir.path().join("b");

    file::put(&a_path, b"dupe").unwrap();

    fs::hard_link(&a_path, &b_path).unwrap();

    let mut d = Scanner::new();
    d.settings.ignore_small = false;
    d.settings.run_mode = RunMode::DryRun;
    d.scan(dir.path()).unwrap();
    let dupes = d.dupes();
    assert_eq!(dupes.len(), 1);
    assert_eq!(dupes[0][0].paths.len(), 2);

    let mut d = Scanner::new();
    d.settings.ignore_small = false;
    d.scan(dir.path()).unwrap();
    let dupes = d.dupes();
    assert_eq!(dupes.len(), 1);
    assert_eq!(dupes[0][0].paths.len(), 2);
}
