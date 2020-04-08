use crate::scanner::ScanListener;
use crate::scanner::Scanner;
use crate::scanner::Stats;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Timing {
    // Time in seconds, used to throttle console output
    next_update: u64,
    start_time: Instant,
}

#[derive(Debug)]
pub struct UI {
    timing: Timing,
}

impl UI {
    pub fn new() -> Self {
        UI {
            timing: Timing {
                next_update: 0,
                start_time: Instant::now(),
            },
        }
    }
}

impl ScanListener for UI {
    fn file_scanned(&mut self, path: &Path, stats: &Stats) {
        let elapsed = self.timing.start_time.elapsed().as_secs();
        if elapsed > self.timing.next_update {
            self.timing.next_update = elapsed+1;
            println!("{}+{} dupes. {}+{} files scanned. {}/…",
                stats.dupes, stats.hardlinks, stats.added, stats.skipped,
                path.parent().unwrap_or(path).display());
        }
    }

    #[allow(overlapping_patterns)]
    fn scan_over(&self, _: &Scanner, stats: &Stats, scan_duration: Duration) {
        let nice_duration = match scan_duration.as_secs() {
            x @ 0..=5 => format!("{:.1}s", (x * 1_000_000_000 + u64::from(scan_duration.subsec_nanos())) as f64 / 1_000_000_000f64),
            x @ 5..=59 => format!("{}s", x),
            x => format!("{}m{}s", x / 60, x % 60),
        };
        println!("Dupes found: {}. Existing hardlinks: {}. Scanned: {}. Skipped {}. Total scan duration: {}",
            stats.dupes, stats.hardlinks, stats.added, stats.skipped, nice_duration);
    }

    fn hardlinked(&mut self, src: &Path, dst: &Path) {
        println!("Hardlinked {}", combined_paths(src, dst));
    }

    fn duplicate_found(&mut self, src: &Path, dst: &Path) {
        println!("Found dupe {}", combined_paths(src, dst));
    }
}

fn combined_paths(base: &Path, relativize: &Path) -> String {
    let base: Vec<_> = base.iter().collect();
    let relativize: Vec<_> = relativize.iter().collect();

    let mut out = String::with_capacity(80);
    let mut prefix_len = 0;
    for (comp, _) in base.iter().zip(relativize.iter()).take_while(|&(a, b)| a == b) {
        prefix_len += 1;
        let comp = comp.to_string_lossy();
        out += &comp;
        if comp != "/" {
            out.push('/');
        }
    }

    let suffix: Vec<_> = base.iter().skip(prefix_len).rev().zip(relativize.iter().skip(prefix_len).rev())
        .take_while(|&(a,b)| a==b).map(|(_,b)|b.to_string_lossy()).collect();

    let base_unique: Vec<_> = base[prefix_len..base.len() - suffix.len()].iter().map(|b| b.to_string_lossy()).collect();

    out.push('{');
    if base_unique.is_empty() {
        out.push('.');
    } else {
        out += &base_unique.join("/");
    }
    out += " => ";

    let rel_unique: Vec<_> = relativize[prefix_len..relativize.len() - suffix.len()].iter().map(|b| b.to_string_lossy()).collect();
    if rel_unique.is_empty() {
        out.push('.');
    } else {
        out += &rel_unique.join("/");
    }
    out.push('}');

    for comp in suffix.into_iter().rev() {
        out.push('/');
        out += &comp;
    }
    out
}

#[test]
fn combined_test() {
    use std::path::PathBuf;
    let a: PathBuf = "foo/bar/baz/a.txt".into();
    let b: PathBuf = "foo/baz/quz/zzz/a.txt".into();
    let c: PathBuf = "foo/baz/quz/zzz/b.txt".into();
    let d: PathBuf = "b.txt".into();
    let e: PathBuf = "e.txt".into();
    let f: PathBuf = "/foo/bar/baz/a.txt".into();
    let g: PathBuf = "/foo/baz/quz/zzz/a.txt".into();
    let h: PathBuf = "/foo/b/quz/zzz/a.txt".into();

    assert_eq!(&combined_paths(&a, &b), "foo/{bar/baz => baz/quz/zzz}/a.txt");
    assert_eq!(&combined_paths(&c, &b), "foo/baz/quz/zzz/{b.txt => a.txt}");
    assert_eq!(&combined_paths(&c, &d), "{foo/baz/quz/zzz => .}/b.txt");
    assert_eq!(&combined_paths(&d, &c), "{. => foo/baz/quz/zzz}/b.txt");
    assert_eq!(&combined_paths(&d, &e), "{b.txt => e.txt}");
    assert_eq!(&combined_paths(&f, &g), "/foo/{bar/baz => baz/quz/zzz}/a.txt");
    assert_eq!(&combined_paths(&h, &g), "/foo/{b => baz}/quz/zzz/a.txt");
}
