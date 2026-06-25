//! TODO: DOCS

use std::fs::File;
use std::path::PathBuf;

use gungraun_summary::v6::BenchmarkSummary;

#[test]
fn test_summary() {
    let current = PathBuf::from(file!())
        .parent()
        .unwrap()
        .join("fixtures/summary.json");

    let summary: BenchmarkSummary = serde_json::from_reader(File::open(&current).unwrap()).unwrap();
    let p = summary.profiles.0;
    let _ = p[0].summaries.total.summary;
}
