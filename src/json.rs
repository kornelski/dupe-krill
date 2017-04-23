use dupe::Stats;
use dupe::ScanListener;
use dupe::Scanner;
use std::path::PathBuf;
use std::path::Path;
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
    fn scan_over(&self, scanner: &Scanner, stats: &Stats) {
        let data = JsonSerializable::new(scanner, stats);
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
struct JsonSerializable {
    creator: String,
}

impl JsonSerializable {
    pub fn new(scanner: &Scanner, stats: &Stats) -> Self {
        JsonSerializable {
            creator: String::from("duplicate-kriller 1.0"),
        }
    }
}