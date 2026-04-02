# Specifying Multiple Benches at Once

Multiple benches can be specified at once with the
[`#[benches]`](macros.md#the-benches-attribute) attribute.

## The `#[benches]` Attribute in More Detail

Start with an example:

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(value: Vec<i32>) -> Vec<i32> { value } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;
use my_lib::bubble_sort;

fn setup_worst_case_array(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}

#[library_benchmark]
#[benches::multiple(vec![1], vec![5])]
#[benches::with_setup(args = [1, 5], setup = setup_worst_case_array)]
fn bench_bubble_sort_with_benches_attribute(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_bubble_sort_with_benches_attribute);
# fn main () {
main!(library_benchmark_groups = my_group);
# }
```

Usually the `arguments` are passed directly to the benchmarking function as it
can be seen in the `#[benches::multiple(/* arguments */)]` case. In
`#[benches::with_setup(/* ... */)]`, the arguments are passed to the `setup`
function instead. The above `#[library_benchmark]` is roughly the same as

```rust
# extern crate gungraun;
# mod my_lib { pub fn bubble_sort(value: Vec<i32>) -> Vec<i32> { value } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;
use my_lib::bubble_sort;

fn setup_worst_case_array(start: i32) -> Vec<i32> {
    if start.is_negative() {
        (start..0).rev().collect()
    } else {
        (0..start).rev().collect()
    }
}

#[library_benchmark]
#[bench::multiple_0(vec![1])]
#[bench::multiple_1(vec![5])]
#[bench::with_setup_0(setup_worst_case_array(1))]
#[bench::with_setup_1(setup_worst_case_array(5))]
fn bench_bubble_sort_with_benches_attribute(input: Vec<i32>) -> Vec<i32> {
    black_box(bubble_sort(black_box(input)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_bubble_sort_with_benches_attribute);
# fn main () {
main!(library_benchmark_groups = my_group);
# }
```

but a lot more concise especially if a lot of values are passed to the same
`setup` function.

### The `iter` parameter

Specifying a lot of benchmarks with args (`args = [1, 2, 3, 4, 5, 6, 7]` and so
on) can be cumbersome. Gungraun supports creating multiple benchmarks from an
iterator, more precisely anything that implements [`IntoIterator`] from the
standard library. Each element of the iterator creates a separate benchmark
case. For example creating multiple benchmarks from a range:

```rust
# extern crate gungraun;
# mod my_lib { pub fn u64_to_string(input: u64) -> String { "1".to_owned() } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

#[library_benchmark]
#[benches::from_iter(iter = 0..10)]
fn some_bench(input: u64) -> String {
    black_box(my_lib::u64_to_string(black_box(input)))
}

library_benchmark_group!(name = my_group, benchmarks = some_bench);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

or reading a directory with benchmark fixtures and each returned path is a
separate benchmark.

```rust
# extern crate gungraun;
# mod my_lib { pub fn count_lines_fast(_path: std::path::PathBuf) -> usize { 0 } }
use std::hint::black_box;
use std::path::PathBuf;

use gungraun::{library_benchmark, library_benchmark_group, main};

fn read_dir() -> Vec<PathBuf> {
    std::fs::read_dir("benches/fixtures")
        .unwrap()
        .map(|d| d.unwrap().path())
        .collect()
}

#[library_benchmark]
#[benches::from_iter(iter = read_dir())]
fn bench_count_lines_fast(path: PathBuf) -> usize {
    black_box(my_lib::count_lines_fast(black_box(path)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_count_lines_fast);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

### The `file` parameter

Reading inputs from a file allows for example sharing the same inputs between
different benchmarking frameworks like `criterion` or if you simply have a long
list of inputs you might find it more convenient to read them from a file.

The `file` parameter, exclusive to the `#[benches]` attribute, does exactly that
and reads the specified file line by line creating a benchmark from each line.
The line is passed to the benchmark function as `String` or if the `setup`
parameter is also present to the `setup` function. A small example assuming you
have a file `benches/inputs` (relative paths are interpreted to the workspace
root) with the following content

```text
1
11
111
```

then

```rust,ignore
# extern crate gungraun;
# mod my_lib { pub fn string_to_u64(value: String) -> Result<u64, String> { Ok(1) } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

#[library_benchmark]
#[benches::from_file(file = "benches/inputs")]
fn some_bench(line: String) -> Result<u64, String> {
    black_box(my_lib::string_to_u64(black_box(line)))
}

library_benchmark_group!(name = my_group, benchmarks = some_bench);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

The above is roughly equivalent to the following but with the `args` parameter

```rust
# extern crate gungraun;
# mod my_lib { pub fn string_to_u64(value: String) -> Result<u64, String> { Ok(1) } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

#[library_benchmark]
#[benches::from_args(args = [1.to_string(), 11.to_string(), 111.to_string()])]
fn some_bench(line: String) -> Result<u64, String> {
    black_box(my_lib::string_to_u64(black_box(line)))
}

library_benchmark_group!(name = my_group, benchmarks = some_bench);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

The true power of the `file` parameter comes with the `setup` function because
you can format the lines in the file as you like and convert each line in the
`setup` function to the format as you need it in the benchmark. For example if
you decided to go with a csv like format in the file `benches/inputs`

```text
255;255;255
0;0;0
```

and your library has a function which converts from RGB to HSV color space:

```rust,ignore
# extern crate gungraun;
# mod my_lib { pub fn rgb_to_hsv(a: u8, b: u8, c:u8) -> (u16, u8, u8) { (a.into(), b, c) } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

fn decode_line(line: String) -> (u8, u8, u8) {
    if let &[a, b, c] = line.split(";")
        .map(|s| s.parse::<u8>().unwrap())
        .collect::<Vec<u8>>()
        .as_slice()
    {
        (a, b, c)
    } else {
        panic!("Wrong input format in line '{line}'");
    }
}

#[library_benchmark]
#[benches::from_file(file = "benches/inputs", setup = decode_line)]
fn some_bench((a, b, c): (u8, u8, u8)) -> (u16, u8, u8) {
    black_box(my_lib::rgb_to_hsv(black_box(a), black_box(b), black_box(c)))
}

library_benchmark_group!(name = my_group, benchmarks = some_bench);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

### The `consts` parameter

When benchmarking functions with const generic parameters, use the `consts`
parameter to specify the const values. With `#[benches]`, multiple const values
can specified the same way as the `args` parameter:

```rust
# extern crate gungraun;
# mod my_lib { pub fn create_buffer(_: usize) -> Vec<u8> { vec![] } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

#[library_benchmark]
#[benches::sizes(consts = [256, 512, 1024, 2048])]
fn bench_multiple_sizes<const SIZE: usize>() -> Vec<u8> {
    black_box(my_lib::create_buffer(black_box(SIZE)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_multiple_sizes);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

You can combine `args` and `consts`. When `args` has more elements than
`consts`, the last `consts` value repeats. Conversely when `consts` has more
elements the last `args` value repeats:

```rust
# extern crate gungraun;
# mod my_lib { pub fn process(_: i32, _: usize) -> i32 { 0 } }
use gungraun::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

#[library_benchmark]
#[benches::combined(args = [1, 2, 3], consts = [256, 512])]
fn bench_args_and_const<const SIZE: usize>(value: i32) -> i32 {
    // Creates:
    // - (SIZE=256, value=1)
    // - (SIZE=512, value=2)
    // - (SIZE=512, value=3)  <- last consts repeats
    black_box(my_lib::process(black_box(value), black_box(SIZE)))
}

library_benchmark_group!(name = my_group, benchmarks = bench_args_and_const);
# fn main() {
main!(library_benchmark_groups = my_group);
# }
```

Const expressions are also supported, so `consts = ({ 1 + 20 })` works.

[`IntoIterator`]: https://doc.rust-lang.org/std/iter/trait.IntoIterator.html
