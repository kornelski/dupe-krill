#![allow(dead_code)]

extern crate sha1;

#[cfg(test)]
extern crate tempdir;

mod dupe;
mod file;
mod hasher;
mod lazyfile;
mod metadata;

use dupe::Scanner;
use std::env;
use std::path::PathBuf;

fn main() {
    let mut s = Scanner::new();
    for arg in env::args_os().skip(1) {
        let path: PathBuf = arg.into();
        s.scan(path).unwrap();
    }
}
