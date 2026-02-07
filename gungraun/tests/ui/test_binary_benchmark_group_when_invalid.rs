mod test_binary_benchmark_group_when_empty {
    use gungraun::binary_benchmark_group;

    binary_benchmark_group!();
}

mod test_binary_benchmark_group_when_no_name {
    use gungraun::{binary_benchmark, binary_benchmark_group, Command};

    #[binary_benchmark]
    fn bench() -> Command {
        Command::new("foo")
    }

    binary_benchmark_group!(
        benchmarks =
    );

    binary_benchmark_group!(benchmarks = bench);

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = setup(),
        teardown = teardown(),
        benchmarks =
    );

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = setup(),
        teardown = teardown(),
        benchmarks = bench
    );

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        setup = setup();
        teardown = teardown();
        benchmarks =
    );

    binary_benchmark_group!(
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        setup = setup();
        teardown = teardown();
        benchmarks = bench
    );

    binary_benchmark_group!(benchmarks = [bench]);
    binary_benchmark_group!(benchmarks = bench, bench);
    binary_benchmark_group!(benchmarks = [bench, bench]);
}

mod test_binary_benchmark_group_low_level_when_no_benchmark {
    use gungraun::binary_benchmark_group;

    // comma syntax
    binary_benchmark_group!(
        name = some,
        benchmarks =
    );

    binary_benchmark_group!(
        name = some,
        benchmarks = |group|
    );

    binary_benchmark_group!(
        name = some,
        benchmarks = |group: &mut BinaryBenchmarkGroup|
    );

    // semicolon syntax
    binary_benchmark_group!(
        name = some;
        benchmarks =
    );

    binary_benchmark_group!(
        name = some;
        benchmarks = |group|
    );

    binary_benchmark_group!(
        name = some;
        benchmarks = |group: &mut BinaryBenchmarkGroup|
    );
}

mod test_binary_benchmark_group_when_no_benchmark_argument {
    use gungraun::binary_benchmark_group;

    binary_benchmark_group!(name = some);
    binary_benchmark_group!(name = some,);
    binary_benchmark_group!(
        name = some;
    );
}

fn main() {}
