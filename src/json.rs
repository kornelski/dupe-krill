use dupe::Stats;
use dupe::ScanListener;
use dupe::Scanner;
use file::FileSet;
use std::path::PathBuf;
use std::path::Path;
use std::time::Duration;
use serde_json;

#[derive(Debug)]
pub struct JsonOutput;

impl JsonOutput {
    pub fn new() -> Self {
        JsonOutput
    }
}

impl ScanListener for JsonOutput {
    fn file_scanned(&mut self, _: &PathBuf, _: &Stats) {
        // output only at scan_over
    }
    fn scan_over(&self, scanner: &Scanner, stats: &Stats, scan_duration: Duration) {
        let data = JsonSerializable::new(scanner, stats, scan_duration);
        let json_string = serde_json::to_string(&data).unwrap();
        println!("{}", json_string);
    }
    fn hardlinked(&mut self, _: &Path, _: &Path) {
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
    dupes: Vec<FileSet>,
    stats: Stats,
    scan_duration: Duration,
}

impl JsonSerializable {
    pub fn new(scanner: &Scanner, stats: &Stats, scan_duration: Duration) -> Self {
        JsonSerializable {
            creator: format!("duplicate-kriller {}", env!("CARGO_PKG_VERSION")),
            dupes: scanner.dupes().into_iter().filter(|x| x.paths.len() > 1).collect(),
            stats: *stats,
            scan_duration: scan_duration,
        }
    }
}