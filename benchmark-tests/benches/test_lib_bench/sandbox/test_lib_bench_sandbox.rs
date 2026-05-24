use std::hint::black_box;
use std::path::Path;

use gungraun::Sandbox;
use gungraun::prelude::*;

fn check_file_exists(path: &str, should_exist: bool) {
    if should_exist {
        assert!(Path::new(path).is_file());
        println!("File exists: '{path}'");
    } else {
        assert!(!Path::new(path).exists());
        println!("File does not exist: '{path}'");
    }
}

#[library_benchmark]
#[bench::when_true_with_fixture(
    args = ("one_line.fix", true),
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["benchmark-tests/benches/fixtures/one_line.fix"])
        )
)]
#[bench::when_true_without_fixture(
    args = ("one_line.fix", false),
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true))
)]
#[bench::when_false_with_fixture(
    args = ("one_line.fix", false),
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(false)
            // Specifying fixtures should do nothing
            .fixtures(["benchmark-tests/benches/fixtures/one_line.fix"])
        )
)]
#[bench::when_false_without_fixture(
    args = ("benches/fixtures/one_line.fix", true),
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(false))
)]
fn sandbox(path: &str, should_exist: bool) {
    check_file_exists(black_box(path), black_box(should_exist));
}

#[library_benchmark]
#[bench::with_sandbox(
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(true)
            .fixtures(["benchmark-tests/benches/fixtures/foo"])
        )
        .current_dir("foo")
)]
#[bench::without_sandbox(
    config = LibraryBenchmarkConfig::default()
        .sandbox(Sandbox::new(false))
        .current_dir("benches/fixtures/foo")
)]
fn current_dir() {
    check_file_exists(black_box("bar.txt"), black_box(true));
}

library_benchmark_group!(name = my_group, benchmarks = [sandbox, current_dir]);

main!(
    config = LibraryBenchmarkConfig::default().sandbox(Sandbox::new(true)),
    library_benchmark_groups = my_group
);
