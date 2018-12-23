use ctrlc;
use dupe_krill::Scanner;
use dupe_krill::*;
use getopts::Options;
use std::env;
use std::error::Error;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

enum OutputMode {
    Quiet,
    Text,
    Json,
}
static CTRL_C_BREAKS: AtomicUsize = std::sync::atomic::ATOMIC_USIZE_INIT;

fn main() {
    let mut opts = Options::new();
    opts.optflag("d", "dry-run", "Do not change anything on disk. Only print dupes found");
    opts.optflag("s", "small", "Also dedupe small files (smaller than a disk block)");
    opts.optflag("q", "quiet", "Hide regular progress output");
    opts.optmulti("e", "exclude", "Don't scan directories or files with that filename (wildcards are not supported)", "<exact filename>");
    opts.optflag("", "json", "Display results as JSON");
    opts.optflag("h", "help", "This help text");

    let mut args = env::args();
    let program = args.next().unwrap_or(env!("CARGO_PKG_NAME").to_owned());

    let matches = opts.parse(args).unwrap();
    let output_mode = if matches.opt_present("json") {
        OutputMode::Json
    } else if matches.opt_present("quiet") {
        OutputMode::Quiet
    } else {
        OutputMode::Text
    };

    if matches.opt_present("h") || matches.free.is_empty() {
        println!(
            "Hardlink files with duplicate content (v{}).\n{}\n\n{}",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_HOMEPAGE"),
            opts.usage(&(opts.short_usage(&program) + " <files or directories>"))
        );
        return;
    }

    ctrlc::set_handler(move || {
        CTRL_C_BREAKS.fetch_add(1, Ordering::SeqCst);
    })
    .ok();

    let mut s = Scanner::new();
    s.settings.break_on = Some(&CTRL_C_BREAKS);
    s.settings.run_mode = if matches.opt_present("dry-run") { RunMode::DryRun } else { RunMode::Hardlink };
    s.settings.ignore_small = !matches.opt_present("small");
    match output_mode {
        OutputMode::Quiet => {
            // Noop-output is already set by default.
        },
        OutputMode::Text => {
            // TODO this print statement belongs into the TextUserInterface.
            if s.settings.run_mode == RunMode::DryRun {
                println!("Dry run. No files will be changed.");
            }
            s.set_listener(Box::new(TextUserInterface::new()));
        },
        OutputMode::Json => {
            if s.settings.run_mode == RunMode::DryRun {
                s.settings.run_mode = RunMode::DryRunNoMerging;
            }
            if cfg!(feature = "json") {
                #[cfg(feature = "json")]
                s.set_listener(Box::new(JsonOutput::new()))
            } else {
                writeln!(&mut std::io::stderr(), "This binary was compiled without JSON support.").unwrap();
                std::process::exit(2)
            }
        },
    }

    s.exclude(matches.opt_strs("exclude"));

    match inner_main(s, matches.free) {
        Ok(()) => {},
        Err(err) => {
            writeln!(&mut std::io::stderr(), "Error: {}; {}", err, err.description()).unwrap();
            std::process::exit(1);
        },
    };
}

fn inner_main(mut s: Scanner, args: Vec<String>) -> io::Result<()> {
    for arg in args {
        let path: PathBuf = arg.into();
        s.enqueue(path)?;
    }
    s.flush()
}
