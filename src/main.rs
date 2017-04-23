extern crate getopts;
extern crate duplicate_kriller;

use duplicate_kriller::Scanner;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use getopts::Options;
use duplicate_kriller::*;
use std::io::Write;

enum OutputMode {
    Quiet,
    Text,
    Json,
}

enum OutputModeParseError {
    UnknownOutputMode(String),
}

impl FromStr for OutputMode {
    type Err = OutputModeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use OutputMode::*;
        match s.to_lowercase().as_ref() {
            "quiet" => Ok(Quiet),
            "text" => Ok(Text),
            "json" => Ok(Json),
            s => Err(OutputModeParseError::UnknownOutputMode(String::from(s))),
        }
    }
}

fn main() {
    let mut opts = Options::new();
    opts.optflag("h", "help", "This help text");
    opts.optflag("d", "dry-run", "Do not change anything on disk. Only print duplicates found");
    opts.optflag("s", "small", "Also dedupe small files (smaller than a disk block)");
    opts.optopt("o", "output-mode", "How to show the results. Valid values are 'quiet', 'text' and 'json'. Default is 'text'", "MODE");

    let mut args = env::args();
    let program = args.next().unwrap_or(env!("CARGO_PKG_NAME").to_owned());

    let matches = opts.parse(args).unwrap();
    let output_mode : OutputMode = matches.opt_str("output-mode").unwrap_or(String::from("text")).parse().unwrap_or_else(|e| match e {
        OutputModeParseError::UnknownOutputMode(s) => {
            writeln!(&mut std::io::stderr(), "Unknown output mode: {:?}", s).unwrap();
            std::process::exit(1)
        }
    });

    if matches.opt_present("h") || matches.free.is_empty() {
        println!("Hardlink files with duplicate content (v{}).\n{}\n\n{}",
            env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_HOMEPAGE"),
            opts.usage(&(opts.short_usage(&program) + " <files or directories>")));
        return;
    }

    let mut s = Scanner::new();
    s.settings.run_mode = if matches.opt_present("dry-run") {RunMode::DryRun} else {RunMode::Hardlink};
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
        }
        OutputMode::Json => {
            if cfg!(feature = "json") {
                #[cfg(feature = "json")]
                s.set_listener(Box::new(JsonOutput::new()))
            } else {
                writeln!(&mut std::io::stderr(), "This binary was compiled without JSON support.").unwrap();
                std::process::exit(2)
            }
        }
    }

    for arg in matches.free {
        let path: PathBuf = arg.into();
        s.enqueue(path).unwrap();
    }
    s.flush().unwrap();
}
