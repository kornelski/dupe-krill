extern crate getopts;
extern crate duplicate_kriller;

use duplicate_kriller::Scanner;
use std::env;
use std::path::PathBuf;
use getopts::Options;
use duplicate_kriller::TextUserInterface;

fn main() {
    let mut opts = Options::new();
    opts.optflag("h", "help", "This help text");
    opts.optflag("d", "dry-run", "Do not change anything on disk. Only print duplicates found");
    opts.optflag("s", "small", "Also dedupe small files (smaller than a disk block)");

    let mut args = env::args();
    let program = args.next().unwrap_or(env!("CARGO_PKG_NAME").to_owned());

    let matches = opts.parse(args).unwrap();

    if matches.opt_present("h") || matches.free.is_empty() {
        println!("Hardlink files with duplicate content (v{}).\n{}\n\n{}",
            env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_HOMEPAGE"),
            opts.usage(&(opts.short_usage(&program) + " <files or directories>")));
        return;
    }

    let mut s = Scanner::new();
    s.set_listener(Box::new(TextUserInterface::new()));
    s.settings.dry_run = matches.opt_present("dry-run");
    s.settings.ignore_small = !matches.opt_present("small");

    if s.settings.dry_run {
        println!("Dry run. No files will be changed.");
    }

    for arg in matches.free {
        let path: PathBuf = arg.into();
        s.enqueue(path).unwrap();
    }
    s.flush().unwrap();
}
