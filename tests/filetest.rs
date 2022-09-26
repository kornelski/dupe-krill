use dupe_krill::*;
use std::fs;
use tempdir::TempDir;

#[test]
fn hardlink_of_same_file() {
    let dir = TempDir::new("hardlinktest").unwrap();
    let a_path = dir.path().join("a").into_boxed_path();
    let b_path = dir.path().join("b").into_boxed_path();

    fs::write(&a_path, "hello").unwrap();

    fs::hard_link(&a_path, &b_path).unwrap();

    let a = FileContent::from_path(a_path).unwrap();
    let b = FileContent::from_path(b_path).unwrap();
    assert_eq!(a, b);
    assert_eq!(b, b);
}

#[test]
fn different_files() {
    let dir = TempDir::new("basictest").unwrap();
    let a_path = dir.path().join("a").into_boxed_path();
    let b_path = dir.path().join("b").into_boxed_path();

    fs::write(&a_path, "hello").unwrap();
    fs::write(&b_path, "world").unwrap();

    let a = FileContent::from_path(a_path).unwrap();
    let b = FileContent::from_path(b_path).unwrap();
    assert_eq!(a, a);
    assert_eq!(b, b);
    assert_ne!(a, b);
}

#[test]
fn different_files_big() {
    let dir = TempDir::new("difftest").unwrap();
    let a_path = dir.path().join("a_big").into_boxed_path();
    let b_path = dir.path().join("b_big").into_boxed_path();

    let mut content = vec![0xffu8; 1400_000];

    fs::write(&a_path, &content).unwrap();
    content[1388_888] = 1;
    fs::write(&b_path, content).unwrap();

    let a = FileContent::from_path(a_path).unwrap();
    let b = FileContent::from_path(b_path).unwrap();
    assert_ne!(a, b);
    assert_eq!(a, a);
    assert_eq!(b, b);
}

#[test]
fn same_content() {
    let dir = TempDir::new("sametest").unwrap();
    let a_path = dir.path().join("a").into_boxed_path();
    let b_path = dir.path().join("b").into_boxed_path();

    fs::write(&a_path, "hello").unwrap();
    fs::write(&b_path, "hello").unwrap();

    let a = FileContent::from_path(a_path).unwrap();
    let b = FileContent::from_path(b_path).unwrap();
    assert_eq!(a, a);
    assert_eq!(b, b);
    assert_eq!(a, b);
}

#[test]
fn symlink() {
    let dir = TempDir::new("sametest").unwrap();
    let a_path = dir.path().join("a").into_boxed_path();
    let b_path = dir.path().join("b").into_boxed_path();
    fs::write(&a_path, "hello").unwrap();

    ::std::os::unix::fs::symlink(&a_path, &b_path).unwrap();

    let a = FileContent::from_path(a_path).unwrap();
    let b = FileContent::from_path(b_path).unwrap();

    assert_ne!(a, b);
    assert_eq!(b, b);
}
