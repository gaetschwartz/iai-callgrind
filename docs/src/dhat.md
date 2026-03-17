<!-- markdownlint-disable MD041 MD033 -->

# DHAT: A Dynamic Heap Analysis Tool

## Intro to DHAT

To fully understand DHAT please read the [Valgrind docs][Dhat] of DHAT. Here's
just a short summary and quote from the docs:

> DHAT is primarily a tool for examining how programs use their heap
> allocations. It tracks the allocated blocks, and inspects every memory access
> to find which block, if any, it is to. It presents, on a program point basis,
> information about these blocks such as sizes, lifetimes, numbers of reads and
> writes, and read and write patterns.

The rest of this chapter is dedicated to how DHAT is integrated into Gungraun.

## The DHAT modes

Gungraun supports all three modes `heap` (the default), `copy` and `ad-hoc`
which can be changed on the [command-line](./cli_and_env/basics.md) with
`--dhat-args=--mode=ad-hoc` or in the benchmark itself with `Dhat::args`. Note
that `ad-hoc` mode requires [client requests](./client_requests.md) which have
prerequisites. If running the benchmarks in `ad-hoc` mode, it is highly
recommended to turn off the `EntryPoint` with `EntryPoint::None` (See next
section). However, DHAT is normally run in `heap` mode and it is assumed that
this is the mode used in the next sections.

## The Default Entry Point

The DHAT default entry point `EntryPoint::Default` in library benchmarks behaves
like
[`Callgrind's EntryPoint`](./benchmarks/library_benchmarks/custom_entry_point.md).
This centers the collected metrics shown in the terminal output on the benchmark
function. The entry point is set to `EntryPoint::None` for binary benchmarks.
But, if necessary, the entry point can be turned off or customized in
`Dhat::entry_point`.

Similar to Callgrind's entry point, the default entry point for DHAT excludes
metrics related to
[`setup` and/or `teardown` code](./benchmarks/library_benchmarks/setup_and_teardown.md),
as well as any elements specified in the `args` parameter of the `#[bench]` or
`#[benches]` attributes. This behavior typically aligns with user expectations.
However, DHAT has a unique characteristic: if the benchmarked function uses an
array created in the setup function, the metrics will not capture the reads and
writes to that array. To accurately measure these reads and writes, it is
necessary to set the entry point to the setup function (in this case, the
`setup_worst_case_array` function).

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }

use std::hint::black_box;
use gungraun::{
    library_benchmark, library_benchmark_group, main, Dhat, EntryPoint, LibraryBenchmarkConfig,
};

pub fn setup_worst_case_array(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}


#[library_benchmark]
#[bench::worst_case_3(setup_worst_case_array(3))]
fn bench_library(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(array))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .entry_point(
                EntryPoint::Custom("*::setup_worst_case_array".to_owned())
            )
        ),
    library_benchmark_groups = my_group
);
# }
```

## Usage on the Command-Line

Running DHAT instead of or in addition to Callgrind is pretty straight-forward
and not different to any [other tool](./tools.md):

Either use
[command-line arguments or environment variables](./cli_and_env/basics.md):
`--default-tool=dhat` or `GUNGRAUN_DEFAULT_TOOL=dhat` (replaces callgrind as
default tool) or `--tools=dhat` or `GUNGRAUN_TOOLS=dhat` (runs DHAT in addition
to the default tool).

## Usage in a Benchmark and a Small Example Analysis

Running DHAT in addition to Callgrind can also be carried out in the benchmark
itself with the `Dhat` struct in `LibraryBenchmarkConfig::tool`. We stick to the
example from above. The above benchmark will produce the following metrics:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_dhat::my_group::bench_library</span> <span style="color:#0AA">worst_case_3</span><span style="color:#0AA">:</span><b><span style="color:#00A">vec! [3, 2, 1]</span></b>
<span style="color:#555">  </span><span style="color:#555">=======</span> CALLGRIND <span style="color:#555">====================================================================</span>
<span style="color:#555">  </span>Instructions:                          <b>83</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>L1 Hits:                              <b>110</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>LL Hits:                                <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>RAM Hits:                               <b>3</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total read+write:                     <b>113</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Estimated Cycles:                     <b>215</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                           <b>12</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total blocks:                           <b>1</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax bytes:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Reads bytes:                           <b>24</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Writes bytes:                          <b>36</b>|N/A                  (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.55554s</code></pre>

Analyzing the DHAT data, there are a total of `12 bytes` of allocations (The
vector: `3 * sizeof(i32)` bytes = `3 * 4` bytes) in `1` block during the setup
of the benchmark in `setup_worst_case_array`. That's also `12` bytes of writes
to fill the vector with the values. That makes `24` bytes of reads and `24`
bytes of writes in the `bubble_sort` function. Also, there are no
(de-)allocations of heap memory in `bubble_sort` itself.

## Soft Limits and Hard Limits

Based on that data, we could define for example hard limits (or soft limits or
both whatever you think is appropriate) to ensure `bubble_sort` is not getting
worse than that.

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(_: Vec<i32>) -> Vec<i32> { vec![] } }
use gungraun::{
    library_benchmark, library_benchmark_group, main, LibraryBenchmarkConfig,
    Dhat, DhatMetric
};
use std::hint::black_box;

#[library_benchmark]
#[bench::worst_case_3(
    args = (vec![3, 2, 1]),
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()
            .hard_limits([
                (DhatMetric::ReadsBytes, 24),
                (DhatMetric::WritesBytes, 32)
            ])
        )
)]
fn bench_bubble_sort(array: Vec<i32>) -> Vec<i32> {
    black_box(my_lib::bubble_sort(array))
}

library_benchmark_group!(name = my_group, benchmarks = bench_bubble_sort);

# fn main() {
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Dhat::default()),
    library_benchmark_groups = my_group
);
# }
```

Now, if `bubble_sort` would read more than `24` bytes or if there were more than
`32` bytes of writes during the benchmark, the benchmark would fail and exit
with error.

## Frames and Benchmarking Multi-Threaded Functions

It is possible to specify additional `Dhat::frames` for example when
benchmarking multi-threaded functions. Like in callgrind, each thread/subprocess
in DHAT is treated as a separate unit and thus requires `frames` (the Gungraun
specific approximation of callgrind toggles) in addition to the default entry
point to include the interesting ones in the measurements.

By example. Suppose there's a function in the `benchmark_tests` library
`find_primes_multi_thread(num_threads: usize)` which searches for primes in the
range `0` - `10000 * num_threads`. This multi-threaded function is splitting the
work for each `10000` numbers into a separate thread each calling the
single-threaded function `benchmark_tests::find_primes` which does the actual
work. The inner workings aren't important but this description should be enough
to understand the basic idea.

```rust
# extern crate gungraun;
# mod benchmark_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
use std::hint::black_box;
use gungraun::{
    library_benchmark, library_benchmark_group, main, LibraryBenchmarkConfig,
    ValgrindTool,
};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(ValgrindTool::DHAT)
)]
fn bench_library() -> Vec<u64> {
    black_box(benchmark_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Running the benchmark produces the following output:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_find_primes::my_group::bench_library</span>
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                        <b>11472</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Total blocks:                           <b>9</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-gmax bytes:                    <b>10264</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>4</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Reads bytes:                          <b>808</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Writes bytes:                       <b>10345</b>|N/A                  (<span style="color:#555">No change</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.47449s</code></pre>

The problem here is, that the spawned thread is not included in the metrics.
Looking at the output files of the dhat output in `dh_view.html` (heavily
shortened to safe some space):

```text
Invocation {
  Mode:    heap
  Command: /home/some/project/target/release/deps/lib_bench_find_primes-59f714debd12ac0a --gungraun-run my_group 0 0 lib_bench_find_primes::my_group::bench_library
  PID:     262049
}

Times {
  t-gmax: 2,826,328 instrs (99.56% of program duration)
  t-end:  2,838,795 instrs
}

▼ PP 1/1 (3 children) {
    Total:     47,303 bytes (100%, 16,663.06/Minstr) in 39 blocks (100%, 13.74/Minstr), avg size 1,212.9 bytes, avg lifetime 858,522.46 instrs (30.24% of program duration)
    At t-gmax: 27,261 bytes (100%) in 10 blocks (100%), avg size 2,726.1 bytes
    At t-end:  456 bytes (100%) in 1 blocks (100%), avg size 456 bytes
    Reads:     45,572 bytes (100%, 16,053.29/Minstr), 0.96/byte
    Writes:    48,000 bytes (100%, 16,908.58/Minstr), 1.01/byte
    Allocated at {
      #0: [root]
    }
  }
  ├─▼ PP 1.1/3 (14 children) {
  │     Total:     46,503 bytes (98.31%, 16,381.25/Minstr) in 30 blocks (76.92%, 10.57/Minstr), avg size 1,550.1 bytes, avg lifetime 880,394 instrs (31.01% of program duration)
  │     At t-gmax: 26,925 bytes (98.77%) in 8 blocks (80%), avg size 3,365.63 bytes
  │     At t-end:  456 bytes (100%) in 1 blocks (100%), avg size 456 bytes
  │     Reads:     45,108 bytes (98.98%, 15,889.84/Minstr), 0.97/byte
  │     Writes:    47,640 bytes (99.25%, 16,781.77/Minstr), 1.02/byte
  │     Allocated at {
  │       #1: 0x48F47A8: malloc (in /usr/lib/valgrind/vgpreload_dhat-amd64-linux.so)
  │     }
  │   }
  │   ├── PP 1.1.1/14 {
  │   │     Total:     32,736 bytes (69.2%, 11,531.65/Minstr) in 10 blocks (25.64%, 3.52/Minstr), avg size 3,273.6 bytes, avg lifetime 235,134.1 instrs (8.28% of program duration)
  │   │     Max:       16,384 bytes in 1 blocks, avg size 16,384 bytes
  │   │     At t-gmax: 16,384 bytes (60.1%) in 1 blocks (10%), avg size 16,384 bytes
  │   │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
  │   │     Reads:     26,184 bytes (57.46%, 9,223.63/Minstr), 0.8/byte
  │   │     Writes:    26,184 bytes (54.55%, 9,223.63/Minstr), 0.8/byte
  │   │     Allocated at {
  │   │       ^1: 0x48F47A8: malloc (in /usr/lib/valgrind/vgpreload_dhat-amd64-linux.so)
  │   │       #2: 0x4050CE3: UnknownInlinedFun (alloc.rs:94)
  │   │       #3: 0x4050CE3: UnknownInlinedFun (alloc.rs:189)
  │   │       #4: 0x4050CE3: UnknownInlinedFun (alloc.rs:250)
  │   │       #5: 0x4050CE3: UnknownInlinedFun (mod.rs:476)
  │   │       #6: 0x4050CE3: with_capacity_in<alloc::alloc::Global> (mod.rs:422)
  │   │       #7: 0x4050CE3: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:190)
  │   │       #8: 0x4050CE3: with_capacity_in<u64, alloc::alloc::Global> (mod.rs:929)
  │   │       #9: 0x4050CE3: with_capacity<u64> (mod.rs:500)
  │   │       #10: 0x4050CE3: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, benchmark_tests::find_primes::{closure_env#0}>> (spec_from_iter_nested.rs:31)
  │   │       #11: 0x4050CE3: <alloc::vec::Vec<T> as alloc::vec::spec_from_iter::SpecFromIter<T,I>>::from_iter (spec_from_iter.rs:34)
  │   │       #12: 0x404EF57: from_iter<u64, core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, benchmark_tests::find_primes::{closure_env#0}>> (mod.rs:3633)
  │   │       #13: 0x404EF57: collect<core::iter::adapters::filter::Filter<core::ops::range::RangeInclusive<u64>, benchmark_tests::find_primes::{closure_env#0}>, alloc::vec::Vec<u64, alloc::alloc::Global>> (iterator.rs:2027)
  │   │       #14: 0x404EF57: benchmark_tests::find_primes (lib.rs:31)
  │   │       #15: 0x40504D0: {closure#0} (lib.rs:38)
  │   │       #16: 0x40504D0: std::sys::backtrace::__rust_begin_short_backtrace (backtrace.rs:158)
  │   │       #17: 0x404FAD4: {closure#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>> (mod.rs:559)
  │   │       #18: 0x404FAD4: call_once<alloc::vec::Vec<u64, alloc::alloc::Global>, std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>> (unwind_safe.rs:272)
  │   │       #19: 0x404FAD4: do_call<core::panic::unwind_safe::AssertUnwindSafe<std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panicking.rs:589)
  │   │       #20: 0x404FAD4: catch_unwind<alloc::vec::Vec<u64, alloc::alloc::Global>, core::panic::unwind_safe::AssertUnwindSafe<std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>>> (panicking.rs:552)
  │   │       #21: 0x404FAD4: catch_unwind<core::panic::unwind_safe::AssertUnwindSafe<std::thread::{impl#0}::spawn_unchecked_::{closure#1}::{closure_env#0}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>>>, alloc::vec::Vec<u64, alloc::alloc::Global>> (panic.rs:359)
  │   │       #22: 0x404FAD4: {closure#1}<benchmark_tests::find_primes_multi_thread::{closure_env#0}, alloc::vec::Vec<u64, alloc::alloc::Global>> (mod.rs:557)
  │   │       #23: 0x404FAD4: core::ops::function::FnOnce::call_once{{vtable.shim}} (function.rs:253)
  │   │       #24: 0x408461E: call_once<(), dyn core::ops::function::FnOnce<(), Output=()>, alloc::alloc::Global> (boxed.rs:1971)
  │   │       #25: 0x408461E: std::sys::pal::unix::thread::Thread::new::thread_start (thread.rs:107)
  │   │       #26: 0x49F09CA: ??? (in /usr/lib/libc.so.6)
  │   │       #27: 0x4A74833: clone (in /usr/lib/libc.so.6)
  │   │     }
  │   │   }
  ...
```

The missing metrics of the thread are caused by the default entry point which
only includes the program points with the benchmark function in their call
stack. But, looking closely at the program point `PP 1.1.1/12` and the call
stack, there's no frame of the benchmark function `bench_library` or a `main`
function. As mentioned earlier, this is because the thread is completely
separated by DHAT.

There are multiple ways to go on depending on what we want to measure. To show
two different approaches, at first, I'll go with measuring the benchmark
function with the function spawning the threads (the default entry point which
doesn't have to be specified) and additionally all threads which execute the
`benchmark_tests::find_primes` function.

```rust
# extern crate gungraun;
# mod benchmark_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
use std::hint::black_box;
use gungraun::{
    library_benchmark, library_benchmark_group, main, LibraryBenchmarkConfig,
    ValgrindTool, Dhat
};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(ValgrindTool::DHAT)
        .tool(Dhat::default()
            .frames(["benchmark_tests::find_primes"])
        )
)]
fn bench_library() -> Vec<u64> {
    black_box(benchmark_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Now, the metrics include the spawned thread(s):

<pre><code class="hljs"><span style="color:#0A0">lib_bench_find_primes::my_group::bench_library</span>
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                        <b>44208</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Total blocks:                          <b>19</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-gmax bytes:                    <b>26648</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>5</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Reads bytes:                        <b>26992</b>|N/A                  (<span style="color:#555">No change</span>)
<span style="color:#555">  </span>Writes bytes:                       <b>36529</b>|N/A                  (<span style="color:#555">No change</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.48695s</code></pre>

If we were only interested in the threads themselves, then using
`EntryPoint::Custom` would be one way to do it. Setting a custom entry point is
syntactic sugar for disabling the entry point with `EntryPoint::None` and
specifying a frame with `Dhat::frames`:

```rust
# extern crate gungraun;
# mod benchmark_tests { pub fn find_primes_multi_thread (_: u64) -> Vec<u64> { vec![] } }
use std::hint::black_box;
use gungraun::{
    library_benchmark, library_benchmark_group, main, LibraryBenchmarkConfig,
    ValgrindTool, Dhat, EntryPoint
};

#[library_benchmark(
    config = LibraryBenchmarkConfig::default()
        .default_tool(ValgrindTool::DHAT)
        .tool(Dhat::default()
            .entry_point(
                EntryPoint::Custom("benchmark_tests::find_primes".to_owned())
            )
        )
)]
fn bench_library() -> Vec<u64> {
    black_box(benchmark_tests::find_primes_multi_thread(black_box(1)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_library);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Running this benchmark results in:

<pre><code class="hljs"><span style="color:#0A0">lib_bench_find_primes::my_group::bench_library</span>
<span style="color:#555">  </span><span style="color:#555">=======</span> DHAT <span style="color:#555">=========================================================================</span>
<span style="color:#555">  </span>Total bytes:                        <b>32736</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Total blocks:                          <b>10</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax bytes:                    <b>16384</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-gmax blocks:                       <b>1</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end bytes:                         <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>At t-end blocks:                        <b>0</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Reads bytes:                        <b>26184</b>|N/A                  (<span style="color:#555">*********</span>)
<span style="color:#555">  </span>Writes bytes:                       <b>26184</b>|N/A                  (<span style="color:#555">*********</span>)

Gungraun result: <b><span style="color:#0A0">Ok</span></b>. 1 without regressions; 0 regressed; 0 filtered; 1 benchmarks finished in 0.45178s</code></pre>

To verify our setup, let's compare these numbers with the data of the program
point with the thread of the `dh_view.html` output shown above. Eventually,
these are the same metrics:

```text
  │   ├── PP 1.1.1/12 {
  │   │     Total:     32,736 bytes (69.91%, 11,537.69/Minstr) in 10 blocks (27.03%, 3.52/Minstr), avg size 3,273.6 bytes, avg lifetime 235,111.9 instrs (8.29% of program duration)
  │   │     Max:       16,384 bytes in 1 blocks, avg size 16,384 bytes
  │   │     At t-gmax: 16,384 bytes (61.03%) in 1 blocks (11.11%), avg size 16,384 bytes
  │   │     At t-end:  0 bytes (0%) in 0 blocks (0%), avg size 0 bytes
  │   │     Reads:     26,184 bytes (57.08%, 9,228.46/Minstr), 0.8/byte
  │   │     Writes:    26,184 bytes (54.23%, 9,228.46/Minstr), 0.8/byte
```

[Dhat]: https://valgrind.org/docs/manual/dh-manual.html
