<!-- spell-checker: ignore fixt binstall libtest eprintln usize Gjengset -->
<!-- markdownlint-disable MD041 MD033 -->

<h1 align="center">Gungraun</h1>

<div align="center">High-precision, one-shot and consistent benchmarking framework/harness for Rust. All Valgrind tools at your fingertips.</div>

<br>
<div align="center">
    <a href="https://gungraun.github.io/gungraun">Guide</a>
    |
    <a href="https://docs.rs/crate/gungraun/">Released API Docs</a>
    |
    <a href="https://github.com/gungraun/gungraun/blob/main/CHANGELOG.md">Changelog</a>
</div>
<div align="center">
    <a href="https://github.com/gungraun/gungraun/actions/workflows/cicd.yml">
        <img src="https://github.com/gungraun/gungraun/actions/workflows/cicd.yml/badge.svg" alt="GitHub branch checks state"/>
    </a>
    <a href="https://crates.io/crates/gungraun">
        <img src="https://img.shields.io/crates/v/gungraun.svg" alt="Crates.io"/>
    </a>
    <a href="https://docs.rs/gungraun/">
        <img src="https://docs.rs/gungraun/badge.svg" alt="docs.rs"/>
    </a>
</div>
<br>

Gungraun leverages Valgrind's profiling tools like [Callgrind][Callgrind
Manual], [Cachegrind][Cachegrind Manual] and [DHAT][DHAT Manual] to provide
extremely accurate and consistent measurements of Rust code, making it perfectly
suited to run in environments like a CI. Gungraun aids in analyzing and
optimizing code paths from the source code level down to the assembly
instruction level.

Gungraun is:

- **Precise**: High-precision measurements of `Instruction` counts, `Estimated
  Cycles` and many other metrics allow you to reliably detect very small
  optimizations and regressions of your code.
- **Consistent**: Gungraun can take accurate measurements even in virtualized CI
  environments and make them comparable between different systems completely
  negating the noise of the environment.
- **Fast**: Each benchmark is only run once, which is usually much faster than
  benchmarks which measure execution and wall-clock time. Benchmarks measuring
  the wall-clock time have to be run many times to increase their accuracy,
  detect outliers, filter out noise, etc.
- **Visualizable**: Gungraun generates a Callgrind (DHAT, ...) profile of the
  benchmarked code and can be configured to create flamegraph-like charts from
  Callgrind metrics. In general, all Valgrind-compatible tools like
  [callgrind_annotate][Callgrind Annotate], [kcachegrind] or `dh_view.html` and
  others to analyze the results in detail are fully supported.
- **Easy**: The API for setting up benchmarks is easy to use and allows you to
  quickly create concise and clear benchmarks. Focus more on profiling and your
  code than on the framework.

See the [Guide] and api documentation at [docs.rs][Api Docs] for all the
details.

## Quickstart/Documentation

To get started read the [Guide] and see some introductory examples in
[Quickstart for library
benchmarks](https://gungraun.github.io/gungraun/latest/html/benchmarks/library_benchmarks/quickstart.html)
or [Quickstart for binary
benchmarks](https://gungraun.github.io/gungraun/latest/html/benchmarks/binary_benchmarks/quickstart.html).
A small migration check-list can be found in the [Guide][Migration checklist].

If you need help or have questions, don't hesitate to [open an issue] or a
[discussion](https://github.com/gungraun/gungraun/discussions)

## Design philosophy and goals

Gungraun benchmarks are designed to be runnable with `cargo bench`. The
benchmark files are expanded to a benchmarking harness which replaces the native
benchmark harness of Rust. Gungraun is a benchmarking and profiling framework
that can quickly and reliably detect performance regressions and optimizations
even in noisy environments with a precision that is impossible to achieve with
wall-clock time based benchmarks. At the same time, we want to abstract the
complicated parts and repetitive tasks away and provide an easy to use and
intuitive api. Gungraun tries to stay out of your way and applies sensible
default settings so you can focus more on profiling and your code!

## How far are we?

Gungraun is in a mature development stage and is already [in
use](https://github.com/gungraun/gungraun/network/dependents). Nevertheless, you
may experience big changes between a minor version bump. The main profiling
tools `Callgrind`, `Cachegrind` and `DHAT` are fully integrated, with full
support for benchmarking async, multi-threaded and multi-process applications.
Please read our [Vision](./VISION.md) to learn more about the ideas and the
direction the future path might take.

## When not to use Gungraun

Although Gungraun is useful in many projects, there are cases where
Gungraun is not a good fit.

- If you need wall-clock times, Gungraun cannot help you much. The estimation of
  cpu cycles merely correlates to wall-clock times but is not a replacement for
  wall-clock times. The cycles estimation is primarily designed to be a relative
  metric to be used for comparison.
- Gungraun cannot be run on Windows and platforms not supported by Valgrind.

## Contributing

Thanks for helping to improve this project! A guideline about contributing to
Gungraun can be found in the [CONTRIBUTING.md](./CONTRIBUTING.md) file.

You have an idea for a new feature, are missing a functionality or have found a
bug? [Open an issue].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed as in
[License](#license), without any additional terms or conditions.

## Pronunciation and origin of Gungraun

Like `valgrind`, the name has its roots in old norse mythology and is a
composition of two words. The first is `gungnir`, Odin's legendary spear, in the
sense of one shot (benchmark execution) and one hit never missing its target.
The second word is `raun` which simply means test.

The first syllable is pronounced like the english word `gun`. The second `g` is
silent. The last syllable `raun` can be pronounced like the english word `rain`.


## Links

- Gungraun/Iai-Callgrind is [mentioned](https://youtu.be/qfknfCsICUM?t=1228) in
  a talk at [RustNation UK](https://www.rustnationuk.com/) about [Towards
  Impeccable Rust](https://www.youtube.com/watch?v=qfknfCsICUM) by Jon Gjengset
- Gungraun/Iai-Callgrind is supported by [Bencher]

## Related Projects

- [Criterion-rs](https://github.com/bheisler/criterion.rs): A Statistics-driven
  benchmarking library for Rust. Wall-clock times based benchmarks.
- [hyperfine](https://github.com/sharkdp/hyperfine): A command-line benchmarking
  tool. Wall-clock time based benchmarks.
- [divan](https://github.com/nvzqz/divan): Statistically-comfy benchmarking
  library. Wall-clock times based benchmarks.
- [dhat-rs](https://github.com/nnethercote/dhat-rs): Provides heap profiling and
  ad hoc profiling capabilities to Rust programs, similar to those provided by
  DHAT.
- [cargo-valgrind](https://github.com/jfrimmel/cargo-valgrind): A cargo
  subcommand, that runs valgrind and collects its output in a helpful manner.

## Credits

Gungraun is forked from <https://github.com/bheisler/iai> and the original idea
is from Brook Heisler (@bheisler).

Gungraun is powered by [Valgrind].

## License

Gungraun is like Iai dual licensed under the Apache 2.0 license and the MIT
license at your option.

According to [Valgrind's documentation][Valgrind Client Request Mechanism]:

> The Valgrind headers, unlike most of the rest of
> the code, are under a BSD-style license, so you may include them without worrying
> about license incompatibility.

We have included the original license where we made use of the original header
files.

[Api Docs]: https://docs.rs/gungraun/latest/gungraun/

[Bencher]: https://bencher.dev/learn/benchmarking/rust/iai/

[Guide]: https://gungraun.github.io/gungraun/

[Migration checklist]: https://gungraun.github.io/gungraun/latest/html/migration/iai-callgrind-to-gungraun.html

[kcachegrind]: https://kcachegrind.github.io/html/Home.html

[Valgrind]: https://valgrind.org/

[Valgrind Client Request Mechanism]: https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq

[Callgrind Manual]: https://valgrind.org/docs/manual/cl-manual.html

[Cachegrind Manual]: https://valgrind.org/docs/manual/cg-manual.html

[DHAT Manual]: https://valgrind.org/docs/manual/dh-manual.html

[Callgrind Annotate]: https://valgrind.org/docs/manual/cl-manual.html#cl-manual.callgrind_annotate-options

[open an issue]: https://github.com/gungraun/gungraun/issues
