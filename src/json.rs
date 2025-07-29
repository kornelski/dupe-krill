use crate::scanner::ScanListener;
use crate::scanner::Scanner;
use crate::scanner::Stats;
use serde_derive::*;
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub struct JsonOutput;

impl JsonOutput {
    pub fn new() -> Self {
        JsonOutput
    }
}

impl ScanListener for JsonOutput {
    fn file_scanned(&mut self, _: &Path, _: &Stats) {
        // output only at scan_over
    }

    fn scan_over(&self, scanner: &Scanner, stats: &Stats, scan_duration: Duration) {
        let data = JsonSerializable::new(scanner, stats, scan_duration);
        let json_string = serde_json::to_string_pretty(&data).unwrap();
        println!("{}", json_string);
    }

    fn hardlinked(&mut self, _: &Path, _: &Path) {
        // output only at scan_over
    }

    fn reflinked(&mut self, _: &Path, _: &Path) {
        // output only at scan_over
    }

    fn duplicate_found(&mut self, _: &Path, _: &Path) {
        // output only at scan_over
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSerializable {
    creator: String,
    dupes: Vec<Vec<Vec<Box<Path>>>>,
    stats: Stats,
    scan_duration: Duration,
}

impl JsonSerializable {
    pub fn new(scanner: &Scanner, stats: &Stats, scan_duration: Duration) -> Self {
        JsonSerializable {
            creator: format!("duplicate-kriller {}", env!("CARGO_PKG_VERSION")),
            dupes: scanner
                .dupes()
                .into_iter()
                .map(|sets| {
                    sets.into_iter()
                        .filter(|set| !set.paths.is_empty())
                        .map(|set| set.paths.into_vec())
                        .collect::<Vec<_>>()
                })
                .filter(|sets| sets.len() > 1 || sets.iter().any(|set| set.len() > 1))
                .collect(),
            stats: *stats,
            scan_duration,
        }
    }
}
