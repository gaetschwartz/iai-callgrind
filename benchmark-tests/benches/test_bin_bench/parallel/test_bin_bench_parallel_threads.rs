use gungraun::{binary_benchmark, binary_benchmark_group, main, Command};

#[binary_benchmark]
fn threads() -> Command {
    Command::new(env!("CARGO_BIN_EXE_thread")).arg("3").build()
}

binary_benchmark_group!(name = my_group, benchmarks = threads);
main!(binary_benchmark_groups = my_group);
