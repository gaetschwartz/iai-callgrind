mod test_library_benchmark_group_when_empty {
    use gungraun::library_benchmark_group;

    library_benchmark_group!();
}

mod test_library_benchmark_group_when_no_name {
    use gungraun::{library_benchmark, library_benchmark_group};

    #[library_benchmark]
    fn some_func() {}

    library_benchmark_group!(benchmarks = some_func);

    // comma syntax
    library_benchmark_group!(
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = setup(),
        teardown = teardown(),
        benchmarks = some_func
    );
    // semicolon syntax
    library_benchmark_group!(
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        setup = setup();
        teardown = teardown();
        benchmarks = some_func
    );

    library_benchmark_group!(benchmarks = [some_func]);
    library_benchmark_group!(benchmarks = some_func, some_func);
    library_benchmark_group!(benchmarks = [some_func, some_func]);
}

mod test_library_benchmark_group_when_no_benchmark_argument {
    use gungraun::library_benchmark_group;

    library_benchmark_group!(name = some_name);
}

mod test_library_benchmark_group_when_no_benchmarks {
    use gungraun::library_benchmark_group;

    // comma syntax
    library_benchmark_group!(
        name = some_name,
        benchmarks =
    );
    library_benchmark_group!(
        name = some_name,
        config = LibraryBenchmarkConfig::default(),
        compare_by_id = true,
        setup = setup(),
        teardown = teardown(),
        benchmarks =
    );

    // semicolon syntax
    library_benchmark_group!(
        name = some_name;
        benchmarks =
    );
    library_benchmark_group!(
        name = some_name;
        config = LibraryBenchmarkConfig::default();
        compare_by_id = true;
        setup = setup();
        teardown = teardown();
        benchmarks =
    );
}

mod test_library_benchmark_group_when_unknown_token {
    use gungraun::library_benchmark_group;

    library_benchmark_group!(something);
}

fn main() {}
