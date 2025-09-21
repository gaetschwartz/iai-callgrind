mod my_lib {
    pub use benchmark_tests::bubble_sort;
}
use std::hint::black_box;

use benchmark_tests::setup_worst_case_array;
use gungraun::{
    library_benchmark, library_benchmark_group, main, Dhat, EntryPoint, LibraryBenchmarkConfig,
};

#[library_benchmark]
#[bench::worst_case_3(setup_worst_case_array(3))]
fn bench_library(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(array))
}

library_benchmark_group!(name = my_group; benchmarks = bench_library);

main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .entry_point(
                EntryPoint::Custom("*::setup_worst_case_array".to_owned())
            )
        );
    library_benchmark_groups = my_group
);
