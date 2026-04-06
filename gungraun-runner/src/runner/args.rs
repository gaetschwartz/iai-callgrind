//! The command-line arguments of cargo bench as in ARGS of `cargo bench -- ARGS`

// spell-checker: ignore totalbytes totalblocks writeback writebackbehaviour

/// Default values for command-line arguments
///
/// This module contains constants that define the default behavior when corresponding command-line
/// arguments are not specified.
pub mod defaults {
    /// Default value for `--allow-aslr`
    ///
    /// When `false` (the default), Gungraun attempts to disable Address Space Layout Randomization
    /// (ASLR) for more consistent benchmark results by using `setarch` on Linux or `proccontrol`
    /// on FreeBSD.
    pub const ALLOW_ASLR: bool = false;

    /// Default value for `--env-clear`
    ///
    /// When `true` (the default), Gungraun clears most environment variables before running the
    /// benchmark. Only essential variables like `LD_PRELOAD`, `LD_LIBRARY_PATH` are preserved.
    pub const ENV_CLEAR: bool = true;
}

use std::ffi::OsString;
use std::fmt::Display;
use std::hash::Hash;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::Result;
use clap::builder::{BoolishValueParser, PathBufValueParser, TypedValueParser};
use clap::{ArgAction, Parser};
use indexmap::{indexset, IndexMap, IndexSet};
use simplematch::{DoWild, Options};
use strum::IntoEnumIterator;

use super::cachegrind::regression::CachegrindRegressionConfig;
use super::callgrind::regression::CallgrindRegressionConfig;
use super::dhat::regression::DhatRegressionConfig;
use super::format::OutputFormatKind;
use super::metrics::{Metric, TypeChecker};
use super::summary::{BaselineName, SummaryFormat};
use super::tool::regression::ToolRegressionConfig;
use crate::api::{
    CachegrindMetric, CachegrindMetrics, CallgrindMetrics, DhatMetric, DhatMetrics, ErrorMetric,
    EventKind, RawToolArgs, ValgrindTool,
};
use crate::runner::common::CapturedOutput;
use crate::util;

const DOWILD_OPTIONS: Options<u8> = Options::new().enable_escape(true).enable_classes(true);

// Utility for complex types intended to be used during the parsing of the command-line arguments
type Limits<T> = (IndexMap<T, f64>, IndexMap<T, Metric>);
type ParsedMetrics<T> = Result<Vec<(T, Option<Metric>)>, String>;

/// A filter for benchmarks
///
/// # Developer Notes
///
/// This enum is used instead of a plain `String` for possible future usages to filter by benchmark
/// ids, group name, file name etc.
#[derive(Debug, Clone)]
pub enum BenchmarkFilter {
    /// The name of the benchmark
    WildcardPattern(String),
}

/// The `NoCapture` options for the command-line argument --nocapture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoCapture {
    /// Don't capture any output
    True,
    /// Capture all output
    False,
    /// Don't capture `stderr`
    Stderr,
    /// Don't capture `stdout`
    Stdout,
}

/// An internal enum for the value of the --truncate-description argument
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncateDescription {
    /// Truncate the description to this value
    To(usize),
    /// Do not truncate the description
    None,
}

/// The command line arguments the user provided after `--` when running cargo bench
///
/// These arguments are not the command line arguments passed to `gungraun-runner`. We collect
/// the command line arguments in the `gungraun::main!` macro without the binary as first
/// argument, that's why `no_binary_name` is set to `true`.
#[allow(clippy::partial_pub_fields, clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "High-precision, one-shot and consistent benchmarking framework/harness for Rust

Boolish command line arguments take also one of `y`, `yes`, `t`, `true`, `on`, `1`
instead of `true` and one of `n`, `no`, `f`, `false`, `off`, and `0` instead of
`false`",
    after_help = "  Exit codes:
      0: Success
      1: All other errors
      2: Parsing command-line arguments failed
      3: One or more regressions occurred
    ",
    long_about = None,
    no_binary_name = true,
    override_usage= "cargo bench ... [FILTER] -- [OPTIONS]",
    max_term_width = 101
)]
pub struct CommandLineArgs {
    ////////////////////////////////////////////////////////////////////////////////////////////////
    // The following arguments are accepted by the rust libtest harness and ignored by us
    //
    // Further details in <https://doc.rust-lang.org/rustc/tests/index.html#cli-arguments> or by
    // running `cargo test -- --help`
    // `--bench` also shows up as last argument set by `cargo bench` even if not explicitly given
    ////////////////////////////////////////////////////////////////////////////////////////////////
    #[arg(long = "bench", hide = true, action = ArgAction::SetTrue, required = false)]
    _bench: bool,

    #[arg(long = "color", hide = true, required = false, num_args = 0..)]
    _color: Vec<String>,

    #[arg(long = "ensure-time", hide = true, action = ArgAction::SetTrue, required = false)]
    _ensure_time: bool,

    #[arg(long = "exact", hide = true, action = ArgAction::SetTrue, required = false)]
    _exact: bool,

    #[arg(
        long = "exclude-should-panic",
        hide = true,
        action = ArgAction::SetTrue,
        required = false
    )]
    _exclude_should_panic: bool,

    #[arg(
        long = "force-run-in-process",
        hide = true,
        action = ArgAction::SetTrue,
        required = false
    )]
    _force_run_in_process: bool,

    #[arg(long = "format", hide = true, required = false, num_args = 0..)]
    _format: Vec<String>,

    #[arg(long = "ignored", hide = true, action = ArgAction::SetTrue, required = false)]
    _ignored: bool,

    #[arg(long = "include-ignored", hide = true, action = ArgAction::SetTrue, required = false)]
    _include_ignored: bool,

    #[arg(long = "logfile", hide = true, required = false, num_args = 0..)]
    _logfile: Vec<String>,

    #[arg(long = "quiet", short = 'q', hide = true, action = ArgAction::SetTrue, required = false)]
    _quiet: bool,

    #[arg(long = "report-time", hide = true, action = ArgAction::SetTrue, required = false)]
    _report_time: bool,

    #[arg(long = "show-output", hide = true, action = ArgAction::SetTrue, required = false)]
    _show_output: bool,

    #[arg(long = "shuffle", hide = true, action = ArgAction::SetTrue, required = false)]
    _shuffle: bool,

    #[arg(long = "shuffle-seed", hide = true, required = false, num_args = 0..)]
    _shuffle_seed: Vec<String>,

    #[arg(long = "skip", hide = true, required = false, num_args = 0..)]
    _skip: Vec<String>,

    #[arg(long = "test", hide = true, action = ArgAction::SetTrue, required = false)]
    _test: bool,

    #[arg(long = "test-threads", hide = true, required = false, num_args = 0..)]
    _test_threads: Vec<String>,

    #[arg(short = 'Z', hide = true, required = false, num_args = 0..)]
    _unstable_options: Vec<String>,

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // End of ignored libtest arguments
    ////////////////////////////////////////////////////////////////////////////////////////////////
    #[rustfmt::skip]
    /// Allow ASLR (Address Space Layout Randomization)
    ///
    /// If possible, ASLR is disabled on platforms that support it (linux, freebsd) because ASLR
    /// could noise up the callgrind cache simulation results a bit. Setting this option to true
    /// runs all benchmarks with ASLR enabled.
    ///
    /// See also [kernel.org: randomize_va_space]
    ///
    /// [kernel.org: randomize_va_space]:
    /// https://docs.kernel.org/admin-guide/sysctl/kernel.html#randomize-va-space
    #[arg(
        long = "allow-aslr",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        env = "GUNGRAUN_ALLOW_ASLR",
        display_order = 100
    )]
    pub allow_aslr: Option<bool>,

    #[rustfmt::skip]
    /// Compare against this baseline if present but do not overwrite it
    #[arg(
        long = "baseline",
        default_missing_value = "default",
        num_args = 0..=1,
        require_equals = true,
        env = "GUNGRAUN_BASELINE",
        display_order = 200
    )]
    pub baseline: Option<BaselineName>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to the experimental BBV
    ///
    /// <https://valgrind.org/docs/manual/bbv-manual.html#bbv-manual.usage>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --bbv-args=--interval-size=10000
    ///   * --bbv-args='--interval-size=10000 --instr-count-only=yes'
    #[arg(
        long = "bbv-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_BBV_ARGS",
        display_order = 500
    )]
    pub bbv_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Cachegrind
    ///
    /// <https://valgrind.org/docs/manual/cg-manual.html#cg-manual.cgopts>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --cachegrind-args=--instr-at-start=no
    ///   * --cachegrind-args='--branch-sim=yes --instr-at-start=no'
    #[arg(
        long = "cachegrind-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_CACHEGRIND_ARGS",
        display_order = 500
    )]
    pub cachegrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    #[allow(clippy::doc_markdown)]
    /// Set performance regression limits for specific cachegrind metrics
    ///
    /// This is a `,` separate list of CachegrindMetric=limit or CachegrindMetrics=limit
    /// (key=value) pairs. See the description of --callgrind-limits for the details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetrics.html>
    /// respectively <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetric.html>
    /// for valid metrics and group members.
    ///
    /// See the guide
    /// (<https://gungraun.github.io/gungraun/latest/html/regressions.html>) for all
    /// details or replace the format spec in `--callgrind-limits` with the following:
    ///
    /// group ::= "@" ( "default"
    ///               | "all"
    ///               | ("cachemisses" | "misses" | "ms")
    ///               | ("cachemissrates" | "missrates" | "mr")
    ///               | ("cachehits" | "hits" | "hs")
    ///               | ("cachehitrates" | "hitrates" | "hr")
    ///               | ("cachesim" | "cs")
    ///               | ("branchsim" | "bs")
    ///               )
    /// event ::= CachegrindMetric
    ///
    /// Examples:
    ///   * --cachegrind-limits='ir=0.0%'
    ///   * --cachegrind-limits='ir=10000,EstimatedCycles=10%'
    ///   * --cachegrind-limits='@all=10%,ir=10000,EstimatedCycles=10%'
    #[arg(
        long = "cachegrind-limits",
        num_args = 1,
        verbatim_doc_comment,
        value_parser = parse_cachegrind_limits,
        env = "GUNGRAUN_CACHEGRIND_LIMITS",
        display_order = 600
    )]
    pub cachegrind_limits: Option<ToolRegressionConfig>,

    #[rustfmt::skip]
    /// Define the cachegrind metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of cachegrind metric groups and event kinds which are allowed
    /// to appear in the terminal output of cachegrind.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetrics.html>
    /// respectively
    /// <https://docs.rs/gungraun/latest/gungraun/enum.CachegrindMetric.html> for valid
    /// metrics and group members.
    ///
    /// The `group` names, their abbreviations if present and `event` kinds are exactly the same as
    /// described in the `--cachegrind-limits` option.
    ///
    /// Examples:
    ///   * --cachegrind-metrics='ir' to show only `Instructions`
    ///   * --cachegrind-metrics='@all' to show all possible cachegrind metrics
    ///   * --cachegrind-metrics='@default,@mr' to show cache miss rates in addition to the defaults
    #[arg(
        long = "cachegrind-metrics",
        num_args = 1..,
        required = false,
        verbatim_doc_comment,
        value_parser = parse_cachegrind_metrics,
        env = "GUNGRAUN_CACHEGRIND_METRICS",
        display_order = 700
    )]
    pub cachegrind_metrics: Option<IndexSet<CachegrindMetric>>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Callgrind
    ///
    /// <https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options> and the core valgrind
    /// command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options>. Note that not all
    /// command-line arguments are supported especially the ones which change output paths.
    /// Unsupported arguments will be ignored printing a warning.
    ///
    /// Examples:
    ///   * --callgrind-args=--dump-instr=yes
    ///   * --callgrind-args='--dump-instr=yes --collect-systime=yes'
    #[arg(
        long = "callgrind-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_CALLGRIND_ARGS",
        display_order = 500
    )]
    pub callgrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    #[allow(clippy::doc_markdown)]
    /// Set performance regression limits for specific `EventKinds`
    ///
    /// This is a `,` separate list of EventKind=limit or CallgrindMetrics=limit (key=value) pairs
    /// with the limit being a soft limit if the number suffixed with a `%` or a hard limit if it
    /// is a bare number. It is possible to specify hard and soft limits in one go with the `|`
    /// operator (e.g. `ir=10%|10000`). Groups (CallgrindMetrics) are prefixed with `@`. List of
    /// allowed groups and events with their abbreviations:
    ///
    /// group ::= "@" ( "default"
    ///               | "all"
    ///               | ("cachemisses" | "misses" | "ms")
    ///               | ("cachemissrates" | "missrates" | "mr")
    ///               | ("cachehits" | "hits" | "hs")
    ///               | ("cachehitrates" | "hitrates" | "hr")
    ///               | ("cachesim" | "cs")
    ///               | ("cacheuse" | "cu")
    ///               | ("systemcalls" | "syscalls" | "sc")
    ///               | ("branchsim" | "bs")
    ///               | ("writebackbehaviour" | "writeback" | "wb")
    ///               )
    /// event ::= EventKind
    ///
    /// See the guide (<https://gungraun.github.io/gungraun/latest/html/regressions.html>)
    /// for more details, the docs of `CallgrindMetrics`
    /// (<https://docs.rs/gungraun/latest/gungraun/enum.CallgrindMetrics.html>) and
    /// `EventKind` <https://docs.rs/gungraun/latest/gungraun/enum.EventKind.html> for a
    /// list of metrics and groups with their members.
    ///
    /// A performance regression check for an `EventKind` fails if the limit is exceeded. If
    /// limits are defined and one or more regressions have occurred during the benchmark run,
    /// the whole benchmark is considered to have failed and the program exits with error and
    /// exit code `3`.
    ///
    /// Examples:
    ///   * --callgrind-limits='ir=5.0%'
    ///   * --callgrind-limits='ir=10000,EstimatedCycles=10%'
    ///   * --callgrind-limits='@all=10%,ir=5%|10000'
    #[arg(
        long = "callgrind-limits",
        num_args = 1,
        verbatim_doc_comment,
        value_parser = parse_callgrind_limits,
        env = "GUNGRAUN_CALLGRIND_LIMITS",
        display_order = 600
    )]
    pub callgrind_limits: Option<ToolRegressionConfig>,

    #[rustfmt::skip]
    /// Define the callgrind metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of callgrind metric groups and event kinds which are allowed
    /// to appear in the terminal output of callgrind. Group names need to be prefixed with '@'.
    /// The order matters and the callgrind metrics are shown in their insertion order of this
    /// option. More precisely, in case of duplicate metrics, the first specified one wins.
    ///
    /// The `group` names, their abbreviations if present and `event` kinds are exactly the same as
    /// described in the `--callgrind-limits` option.
    ///
    /// For a list of valid metrics, groups and their members see the docs of `CallgrindMetrics`
    /// (<https://docs.rs/gungraun/latest/gungraun/enum.CallgrindMetrics.html>) and
    /// `EventKind` <https://docs.rs/gungraun/latest/gungraun/enum.EventKind.html>.
    ///
    /// Note that setting the metrics here does not imply that these metrics are actually
    /// collected. This option just sets the order and appearance of metrics in case they are
    /// collected. To activate the collection of specific metrics you need to use
    /// `--callgrind-args`.
    ///
    /// Examples:
    ///   * --callgrind-metrics='ir' to show only `Instructions`
    ///   * --callgrind-metrics='@all' to show all possible callgrind metrics
    ///   * --callgrind-metrics='@default,@mr' to show cache miss rates in addition to the defaults
    #[arg(
        long = "callgrind-metrics",
        num_args = 1..,
        required = false,
        verbatim_doc_comment,
        value_parser = parse_callgrind_metrics,
        env = "GUNGRAUN_CALLGRIND_METRICS",
        display_order = 700
    )]
    pub callgrind_metrics: Option<IndexSet<EventKind>>,

    #[rustfmt::skip]
    /// The default tool used to run the benchmarks
    ///
    /// The standard tool to run the benchmarks is callgrind but can be overridden with this
    /// option. Any valgrind tool can be used:
    ///   * callgrind
    ///   * cachegrind
    ///   * dhat
    ///   * memcheck
    ///   * helgrind
    ///   * drd
    ///   * massif
    ///   * exp-bbv
    ///
    /// This argument matches the tool case-insensitive. Note that using cachegrind with this
    /// option to benchmark library functions needs adjustments to the benchmarking functions with
    /// client-requests to measure the counts correctly. If you want to switch permanently to
    /// cachegrind, it is usually better to activate the `cachegrind` feature of gungraun in
    /// your Cargo.toml. However, setting a tool with this option overrides cachegrind set with the
    /// gungraun feature. See the guide for all details.
    #[arg(
        long = "default-tool",
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_DEFAULT_TOOL",
        display_order = 50
    )]
    pub default_tool: Option<ValgrindTool>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to DHAT
    ///
    /// <https://valgrind.org/docs/manual/dh-manual.html#dh-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --dhat-args=--mode=ad-hoc
    #[arg(
        long = "dhat-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_DHAT_ARGS",
        display_order = 500
    )]
    pub dhat_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    #[allow(clippy::doc_markdown)]
    /// Set performance regression limits for specific dhat metrics
    ///
    /// This is a `,` separate list of DhatMetrics=limit or DhatMetric=limit (key=value) pairs. See
    /// the description of --callgrind-limits for the details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetrics.html> respectively
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetric.html> for valid metrics
    /// and group members.
    ///
    /// See the guide
    /// (<https://gungraun.github.io/gungraun/latest/html/regressions.html>) for all
    /// details or replace the format spec in `--callgrind-limits` with the following:
    ///
    /// group ::= "@" ( "default" | "all" )
    /// event ::=   ( "totalunits" | "tun" )
    ///           | ( "totalevents" | "tev" )
    ///           | ( "totalbytes" | "tb" )
    ///           | ( "totalblocks" | "tbk" )
    ///           | ( "attgmaxbytes" | "gb" )
    ///           | ( "attgmaxblocks" | "gbk" )
    ///           | ( "attendbytes" | "eb" )
    ///           | ( "attendblocks" | "ebk" )
    ///           | ( "readsbytes" | "rb" )
    ///           | ( "writesbytes" | "wb" )
    ///           | ( "totallifetimes" | "tl" )
    ///           | ( "maximumbytes" | "mb" )
    ///           | ( "maximumblocks" | "mbk" )
    ///
    /// `events` with a long name have their allowed abbreviations placed in the same parentheses.
    ///
    /// Examples:
    ///   * --dhat-limits='totalbytes=0.0%'
    ///   * --dhat-limits='totalbytes=10000,totalblocks=5%'
    ///   * --dhat-limits='@all=10%,totalbytes=5000,totalblocks=5%'
    #[arg(
        long = "dhat-limits",
        num_args = 1,
        verbatim_doc_comment,
        value_parser = parse_dhat_limits,
        env = "GUNGRAUN_DHAT_LIMITS",
        display_order = 600
    )]
    pub dhat_limits: Option<ToolRegressionConfig>,

    #[rustfmt::skip]
    /// Define the dhat metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of dhat metric groups and event kinds which are allowed to
    /// appear in the terminal output of dhat.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetrics.html> respectively
    /// <https://docs.rs/gungraun/latest/gungraun/enum.DhatMetric.html> for valid metrics
    /// and group members.
    ///
    /// The `group` names, their abbreviations if present and `event` kinds are exactly the same as
    /// described in the `--dhat-limits` option.
    ///
    /// Examples:
    ///   * --dhat-metrics='totalbytes' to show only `Total Bytes`
    ///   * --dhat-metrics='@all' to show all possible dhat metrics
    ///   * --dhat-metrics='@default,mb' to show maximum bytes in addition to the defaults
    #[arg(
        long = "dhat-metrics",
        num_args = 1..,
        required = false,
        verbatim_doc_comment,
        value_parser = parse_dhat_metrics,
        env = "GUNGRAUN_DHAT_METRICS",
        display_order = 700
    )]
    pub dhat_metrics: Option<IndexSet<DhatMetric>>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to DRD
    ///
    /// <https://valgrind.org/docs/manual/drd-manual.html#drd-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --drd-args=--exclusive-threshold=100
    ///   * --drd-args='--exclusive-threshold=100 --free-is-write=yes'
    #[arg(
        long = "drd-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_DRD_ARGS",
        display_order = 500
    )]
    pub drd_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Define the drd error metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of error metrics which are allowed to appear in the terminal
    /// output of drd. The `group` and `event` are the same as for `--memcheck-metrics`.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid error
    /// metrics.
    ///
    /// Since this is a very small set of metrics, there is only one `group`: `@all`
    ///
    /// Examples:
    ///   * --drd-metrics='errors' to show only `Errors`
    ///   * --drd-metrics='@all' to show all possible error metrics (the default)
    ///   * --drd-metrics='err,ctx' to show only errors and contexts
    #[arg(
        long = "drd-metrics",
        num_args = 1..,
        required = false,
        verbatim_doc_comment,
        value_parser = parse_drd_metrics,
        env = "GUNGRAUN_DRD_METRICS",
        display_order = 700
    )]
    pub drd_metrics: Option<IndexSet<ErrorMetric>>,

    #[rustfmt::skip]
    /// Control whether environment variables are cleared before running a benchmark
    ///
    /// By default (`true`), environment variables are cleared to ensure reproducible benchmark
    /// results across different environments. Set to `false` to preserve all environment variables
    /// of the `cargo bench` process.
    ///
    /// Examples:
    ///   * `--env-clear` (default: clear environment)
    ///   * `--env-clear=false` (preserve environment)
    #[arg(
        long = "env-clear",
        num_args = 0..=1,
        default_missing_value = "true",
        verbatim_doc_comment,
        value_parser = BoolishValueParser::new(),
        env = "GUNGRAUN_ENV_CLEAR",
        display_order = 100
    )]
    pub env_clear: Option<bool>,

    #[rustfmt::skip]
    /// Set environment variables for benchmarks ignoring the clearing of environment variables
    ///
    /// Environment variables can be specified in two forms:
    /// - `KEY=VALUE`: Set `KEY` to `VALUE` explicitly
    /// - `KEY`: Resolve `KEY` from the current environment and pass its value
    ///
    /// Multiple key-value pairs can be specified in a single invocation using space-separated
    /// values (posix-style quoting of values is supported). The `--envs` argument can also be
    /// specified multiple times to accumulate environment variables.
    ///
    /// These variables are cumulative to any environment variables configured via
    /// `LibraryBenchmarkConfig::env` or `BinaryBenchmarkConfig::env`.
    ///
    /// Examples:
    ///   * `--envs=FOO=bar` (set FOO to "bar")
    ///   * `--envs=FOO` (pass the original value of FOO from current environment)
    ///   * `--envs='FOO=bar BAZ=qux'` (set multiple variables and once)
    ///   * `--envs=FOO=bar --envs=BAZ=qux` (accumulate multiple times)
    #[arg(
        long = "envs",
        num_args = 1,
        required = false,
        require_equals = true,
        action = ArgAction::Append,
        verbatim_doc_comment,
        value_parser = parse_envs,
        env = "GUNGRAUN_ENVS",
        display_order = 100
    )]
    pub envs: Vec<Vec<(OsString, OsString)>>,

    #[rustfmt::skip]
    /// If specified, only run benchmarks matching this wildcard pattern
    ///
    /// The wildcard pattern can contain `*` to match any amount of characters, `?` to match a
    /// single character and simple classes `[...]` like `[abc] `to match the characters `a` or `b`
    /// or `c`. Character classes can contain ranges, so `[abc]` could be rewritten as `[a-c]` and
    /// they can be negated with `[!...]` to not match the contained characters.
    ///
    /// This pattern matches the whole module path of benchmarks. A list of all benchmarks with
    /// their module path as recognized by this option can be obtained by running `--list`. The
    /// general structure of the module path of a benchmark is:
    ///
    /// `FILENAME::GROUP::FUNCTION::ID`
    ///
    /// Examples:
    ///   * `*::my_benchmark_id` runs all benchmarks with the id `my_benchmark_id`
    ///   * `gungraun_benchmarks::*` runs all benchmarks in the file `gungraun_benchmarks`
    ///   * `my_file::some_group::*` runs all benchmarks in the file `my_file` and the group
    ///     `some_group`
    #[arg(
        name = "FILTER",
        num_args = 0..=1,
        verbatim_doc_comment,
        env = "GUNGRAUN_FILTER"
    )]
    pub filter: Option<BenchmarkFilter>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Helgrind
    ///
    /// <https://valgrind.org/docs/manual/hg-manual.html#hg-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --helgrind-args=--free-is-write=yes
    ///   * --helgrind-args='--conflict-cache-size=100000 --free-is-write=yes'
    #[arg(
        long = "helgrind-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_HELGRIND_ARGS",
        display_order = 500
    )]
    pub helgrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Define the helgrind error metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of error metrics which are allowed to appear in the terminal
    /// output of helgrind. The `group` and `event` are the same as for `--memcheck-metrics`.
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid error
    /// metrics.
    ///
    /// Examples:
    ///   * --helgrind-metrics='errors' to show only `Errors`
    ///   * --helgrind-metrics='@all' to show all possible error metrics (the default)
    ///   * --helgrind-metrics='err,ctx' to show only errors and contexts
    #[arg(
        long = "helgrind-metrics",
        num_args = 1..,
        required = false,
        verbatim_doc_comment,
        value_parser = parse_helgrind_metrics,
        env = "GUNGRAUN_HELGRIND_METRICS",
        display_order = 700
    )]
    pub helgrind_metrics: Option<IndexSet<ErrorMetric>>,

    #[rustfmt::skip]
    /// Specify the home directory of gungraun benchmark output files
    ///
    /// All output files are by default stored under the `$PROJECT_ROOT/target/gungraun` directory.
    /// This option lets you customize this home directory, and it will be created if it doesn't
    /// exist.
    #[arg(
        long = "home",
        num_args = 1,
        env = "GUNGRAUN_HOME",
        display_order = 100
    )]
    pub home: Option<PathBuf>,

    // FIX: should be exclusive
    #[rustfmt::skip]
    /// Print a list of all benchmarks. With this argument no benchmarks are executed.
    ///
    /// The output format is intended to be the same as the output format of the libtest harness.
    /// However, future changes of the output format by cargo might not be incorporated into
    /// gungraun. As a consequence, it is not considered safe to rely on the output in
    /// scripts.
    #[arg(
        long = "list",
        default_missing_value = "true",
        default_value = "false",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        action = ArgAction::Set,
        env = "GUNGRAUN_LIST"
    )]
    pub list: bool,

    #[rustfmt::skip]
    /// Load this baseline as the new data set instead of creating a new one
    #[clap(
        id = "LOAD_BASELINE",
        long = "load-baseline",
        requires = "baseline",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "default",
        env = "GUNGRAUN_LOAD_BASELINE",
        display_order = 200
    )]
    pub load_baseline: Option<BaselineName>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Massif
    ///
    /// <https://valgrind.org/docs/manual/ms-manual.html#ms-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --massif-args=--heap=no
    ///   * --massif-args='--heap=no --threshold=2.0'
    #[arg(
        long = "massif-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_MASSIF_ARGS",
        display_order = 500
    )]
    pub massif_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to Memcheck
    ///
    /// <https://valgrind.org/docs/manual/mc-manual.html#mc-manual.options>. See also the
    /// description for --callgrind-args for more details and restrictions.
    ///
    /// Examples:
    ///   * --memcheck-args=--leak-check=full
    ///   * --memcheck-args='--leak-check=yes --show-leak-kinds=all'
    #[arg(
        long = "memcheck-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_MEMCHECK_ARGS",
        display_order = 500
    )]
    pub memcheck_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Define the memcheck error metrics and the order in which they are displayed
    ///
    /// This is a `,`-separated list of error metrics which are allowed to appear in the terminal
    /// output of memcheck.
    ///
    /// Since this is a very small set of metrics, there is only one `group`: `@all`
    ///
    /// group ::= "@all"
    /// event ::=   ( "errors" | "err" )
    ///           | ( "contexts" | "ctx" )
    ///           | ( "suppressederrors" | "serr")
    ///           | ( "suppressedcontexts" | "sctx" )
    ///
    /// See `--callgrind-metrics` for more details and
    /// <https://docs.rs/gungraun/latest/gungraun/enum.ErrorMetric.html> for valid
    /// metrics.
    ///
    /// Examples:
    ///   * --memcheck-metrics='errors' to show only `Errors`
    ///   * --memcheck-metrics='@all' to show all possible error metrics (the default)
    ///   * --memcheck-metrics='err,ctx' to show only errors and contexts
    #[arg(
        long = "memcheck-metrics",
        num_args = 1..,
        required = false,
        verbatim_doc_comment,
        value_parser = parse_memcheck_metrics,
        env = "GUNGRAUN_MEMCHECK_METRICS",
        display_order = 700
    )]
    pub memcheck_metrics: Option<IndexSet<ErrorMetric>>,

    // FIX: Add alias --no-capture
    #[rustfmt::skip]
    /// Don't capture terminal output of benchmarks
    ///
    /// Possible values are one of [true, false, stdout, stderr].
    ///
    /// This option is currently restricted to the `callgrind` run of benchmarks. The output of
    /// additional tool runs like DHAT, Memcheck, ... is still captured, to prevent showing the
    /// same output of benchmarks multiple times. Use `GUNGRAUN_LOG=info` to also show
    /// captured and logged output.
    ///
    /// If no value is given, the default missing value is `true` and doesn't capture stdout and
    /// stderr. Besides `true` or `false` you can specify the special values `stdout` or `stderr`.
    /// If `--nocapture=stdout` is given, the output to `stdout` won't be captured and the output
    /// to `stderr` will be discarded. Likewise, if `--nocapture=stderr` is specified, the output
    /// to `stderr` won't be captured and the output to `stdout` will be discarded.
    #[arg(
        long = "nocapture",
        required = false,
        default_missing_value = "true",
        default_value = "false",
        num_args = 0..=1,
        require_equals = true,
        value_parser = parse_nocapture,
        env = "GUNGRAUN_NOCAPTURE",
        display_order = 300
    )]
    pub nocapture: NoCapture,

    // FIX: Add alias no-summary
    #[rustfmt::skip]
    /// Suppress the summary showing regressions and execution time at the end of a benchmark run
    ///
    /// Note, that a summary is only printed if the `--output-format` is not JSON.
    ///
    /// The summary described by `--nosummary` is different from `--save-summary` and they do not
    /// affect each other.
    #[arg(
        long = "nosummary",
        default_missing_value = "true",
        default_value = "false",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        action = ArgAction::Set,
        env = "GUNGRAUN_NOSUMMARY",
        display_order = 300
    )]
    pub nosummary: bool,

    #[rustfmt::skip]
    /// The terminal output format in default human-readable format or in machine-readable json
    /// format
    ///
    /// # The JSON Output Format
    ///
    /// The json terminal output schema is the same as the schema with the `--save-summary`
    /// argument when saving to a `summary.json` file. All other output than the json output goes
    /// to stderr and only the summary output goes to stdout. When not printing pretty json, each
    /// line is a dictionary summarizing a single benchmark. You can combine all lines (benchmarks)
    /// into an array for example with `jq`
    ///
    /// `cargo bench -- --output-format=json | jq -s`
    ///
    /// which transforms `{...}\n{...}` into `[{...},{...}]`
    #[arg(
        long = "output-format",
        value_enum,
        required = false,
        default_value = "default",
        num_args = 1,
        env = "GUNGRAUN_OUTPUT_FORMAT",
        display_order = 300
    )]
    pub output_format: OutputFormatKind,

    #[rustfmt::skip]
    /// Number of benchmarks to run in parallel.
    ///
    /// A value of `1` runs benchmarks serially which is the default if this option is not
    /// specified. Passing `auto` lets the runner choose the parallelism level based on available
    /// hardware which is the number of available logical cores.
    ///
    /// Note that benchmark groups are used as synchronization points and only benchmarks within the
    /// same group are executed in parallel.
    ///
    /// Valgrind and gungraun perform disk I/O even if your benchmarks don't. This is usually a
    /// bottleneck, so running with parallelism of 10 may provide similar speedup as 5. Actual
    /// results depend on the hardware and if your benchmarks are performing disk I/O, too.
    ///
    /// Examples:
    ///   * --parallel=4
    ///   * --parallel=auto
    #[arg(
        long = "parallel",
        required = false,
        default_missing_value = "auto",
        default_value = "1",
        num_args = 0..=1,
        require_equals = true,
        value_parser = parse_parallel,
        env = "GUNGRAUN_PARALLEL",
        display_order = 300 // FIX: DISPLAY ORDER
    )]
    pub parallel: usize,

    #[rustfmt::skip]
    /// If true, the first failed performance regression check fails the whole benchmark run
    ///
    /// Note that if --regression-fail-fast is set to true, no summary is printed.
    #[arg(
        long = "regression-fail-fast",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        env = "GUNGRAUN_REGRESSION_FAIL_FAST",
        display_order = 600
    )]
    pub regression_fail_fast: Option<bool>,

    #[rustfmt::skip]
    /// Compare against this baseline if present and then overwrite it
    #[arg(
        long = "save-baseline",
        default_missing_value = "default",
        num_args = 0..=1,
        require_equals = true,
        conflicts_with_all = &["baseline", "LOAD_BASELINE"],
        env = "GUNGRAUN_SAVE_BASELINE",
        display_order = 200
    )]
    pub save_baseline: Option<BaselineName>,

    #[rustfmt::skip]
    /// Save a machine-readable summary of each benchmark run in json format next to the usual
    /// benchmark output
    #[arg(
        long = "save-summary",
        value_enum,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "json",
        env = "GUNGRAUN_SAVE_SUMMARY",
        display_order = 300
    )]
    pub save_summary: Option<SummaryFormat>,

    #[rustfmt::skip]
    /// Separate gungraun benchmark output files by target
    ///
    /// The default output path for files created by gungraun and valgrind during the
    /// benchmark is
    ///
    /// `target/gungraun/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`.
    ///
    /// This can be problematic if you're running the benchmarks not only for a single target
    /// because you end up comparing the benchmark runs with the wrong targets. Setting this option
    /// changes the default output path to
    ///
    /// `target/gungraun/$TARGET/$PACKAGE_NAME/$BENCHMARK_FILE/$GROUP/$BENCH_FUNCTION.$BENCH_ID`
    ///
    /// Although not as comfortable and strict, you could achieve a separation by target also with
    /// baselines and a combination of `--save-baseline=$TARGET` and `--baseline=$TARGET` if you
    /// prefer having all files of a single $BENCH in the same directory.
    #[arg(
        long = "separate-targets",
        default_missing_value = "true",
        default_value = "false",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        action = ArgAction::Set,
        env = "GUNGRAUN_SEPARATE_TARGETS",
        display_order = 100
    )]
    pub separate_targets: bool,

    #[rustfmt::skip]
    /// Show an ascii grid in the benchmark terminal output
    ///
    /// A matter of taste but the guiding lines can also be helpful reading benchmark output when
    /// running multiple tools with multiple threads and subprocesses for example by using
    /// `--show-intermediate`.
    #[arg(
        long = "show-grid",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        env = "GUNGRAUN_SHOW_GRID",
        display_order = 300
    )]
    pub show_grid: Option<bool>,

    #[rustfmt::skip]
    /// Show intermediate metrics from parts, subprocesses, threads, ... (Default: false)
    ///
    /// In callgrind, threads are treated as separate units (similar to subprocesses) and the
    /// metrics for them are dumped into an own file. Other valgrind tools usually separate the
    /// output files only by subprocesses. Use this option, to also show the metrics of any
    /// intermediate fragments and not just the total over all of them.
    ///
    /// Temporarily setting `show_intermediate` to `true` can help to find misconfigurations in
    /// multi-thread/multi-process benchmarks.
    #[arg(
        long = "show-intermediate",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        env = "GUNGRAUN_SHOW_INTERMEDIATE",
        display_order = 300
    )]
    pub show_intermediate: Option<bool>,

    #[rustfmt::skip]
    /// Show only the comparison between different benchmarks when using `compare_by_id`
    ///
    /// If you're only interested in the comparisons between different benchmarks but not the metric
    /// differences between the self comparisons of the new and old benchmark run, use this option.
    /// This option is only useful if `compare_by_id` is used in the `library_benchmark_group!` or
    /// `binary_benchmark_group!`. Note, that it does not prevent any benchmarks to be run,
    /// especially benchmarks which are not compared to another benchmark. Such benchmarks have only
    /// the usual benchmark headline printed.
    #[arg(
        long = "show-only-comparison",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true,
        value_parser = BoolishValueParser::new(),
        verbatim_doc_comment,
        env = "GUNGRAUN_SHOW_ONLY_COMPARISON",
        display_order = 300
    )]
    pub show_only_comparison: Option<bool>,

    #[rustfmt::skip]
    /// Show changes only when they are above the `tolerance` level
    ///
    /// If no value is specified, the default value of `0.000_009_999_999_999_999_999` is based on
    /// the number of decimal places of the percentages displayed in the terminal output in case of
    /// differences.
    ///
    /// Negative tolerance values are converted to their absolute value.
    ///
    /// Examples:
    ///   * --tolerance (applies the default value)
    ///   * --tolerance=0.1 (set the tolerance level to `0.1`)
    #[arg(
        long = "tolerance",
        default_missing_value = "0.000009999999999999999",
        num_args = 0..=1,
        require_equals = true,
        verbatim_doc_comment,
        env = "GUNGRAUN_TOLERANCE",
        display_order = 300
    )]
    pub tolerance: Option<f64>,

    #[rustfmt::skip]
    /// A comma separated list of tools to run additionally to callgrind or another default tool
    ///
    /// The tools specified here take precedence over the tools in the benchmarks. The valgrind
    /// tools which are allowed here are the same as the ones listed in the documentation of
    /// --default-tool.
    ///
    /// Examples
    ///   * --tools dhat
    ///   * --tools memcheck,drd
    #[arg(
        long = "tools",
        num_args = 1..,
        value_delimiter = ',',
        verbatim_doc_comment,
        env = "GUNGRAUN_TOOLS",
        display_order = 50
    )]
    pub tools: Vec<ValgrindTool>,

    #[rustfmt::skip]
    /// Adjust, enable or disable the truncation of the description in the Gungraun output
    ///
    /// The default is to truncate the description to the size of 50 ascii characters. A false
    /// value disables the truncation entirely and a value will truncate the description to the
    /// given amount of characters excluding the ellipsis.
    ///
    /// To clarify which part of the output is meant by `DESCRIPTION`:
    ///
    /// ```text
    /// benchmark_file::group_name::function_name id:DESCRIPTION
    ///   Instructions:              352135|352135          (No change)
    ///   ...
    /// ```
    ///
    /// Examples:
    ///   * --truncate-description=no (disables truncation)
    ///   * --truncate-description=100 (set the truncation to 100 ascii chars)
    ///   * --truncate-description (this is the default and sets the size of 50 ascii chars)
    #[arg(
        long = "truncate-description",
        default_missing_value = "50",
        num_args = 0..=1,
        require_equals = true,
        value_parser = parse_truncate_description,
        verbatim_doc_comment,
        env = "GUNGRAUN_TRUNCATE_DESCRIPTION",
        display_order = 300
    )]
    pub truncate_description: Option<TruncateDescription>,

    #[rustfmt::skip]
    /// The command-line arguments to pass through to all tools
    ///
    /// The core valgrind command-line arguments
    /// <https://valgrind.org/docs/manual/manual-core.html#manual-core.options> which are
    /// recognized by all tools. More specific arguments for example set with --callgrind-args
    /// override the arguments with the same name specified with this option.
    ///
    /// Examples:
    ///   * --valgrind-args=--time-stamp=yes
    ///   * --valgrind-args='--error-exitcode=202 --num-callers=50'
    #[arg(
        long = "valgrind-args",
        value_parser = parse_tool_args,
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_VALGRIND_ARGS",
        display_order = 500
    )]
    pub valgrind_args: Option<RawToolArgs>,

    #[rustfmt::skip]
    /// Specify the path to the valgrind executable
    ///
    /// By default, Gungraun searches for `valgrind` in the system PATH. This option
    /// allows specifying an alternative valgrind executable. When used with
    /// `--valgrind-runner`, this path is passed to the runner as the valgrind binary
    /// to invoke.
    ///
    /// Note: The specified path is not validated for existence. If the path is invalid, the
    /// benchmark will fail when attempting to execute valgrind.
    ///
    /// Examples:
    ///   * `--valgrind-bin=/usr/local/bin/valgrind`
    ///   * `--valgrind-bin=/doesnotexist` (used with `--valgrind-runner` for container setups)
    #[arg(
        long = "valgrind-bin",
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_VALGRIND_BIN",
        display_order = 500
    )]
    pub valgrind_bin: Option<PathBuf>,

    #[rustfmt::skip]
    /// Specify an alternative executable to run valgrind
    ///
    /// By default, gungraun runs the benchmark executable with valgrind directly. This option
    /// allows specifying an alternative runner executable that will be invoked instead, with
    /// valgrind passed as an argument to the runner.
    ///
    /// When specified, the runner is invoked as:
    ///   `<RUNNER> [RUNNER_ARGS...] <VALGRIND_BIN> [VALGRIND_ARGS...] <BENCHMARK> [BENCHMARK_ARGS...]`
    ///
    /// The runner receives extra environment variables that provide context:
    /// - `GUNGRAUN_VR_DEST_DIR`: The destination directory for valgrind output files
    /// - `GUNGRAUN_VR_HOME`: The gungraun home (`--home`) directory
    /// - `GUNGRAUN_VR_WORKSPACE_ROOT`: The project's workspace root directory
    /// - `GUNGRAUN_ALLOW_ASLR`: `yes` or `no` (the default) based on `--allow-aslr` setting
    ///
    /// Environment variables in `--valgrind-runner-args` are interpolated using `${VAR}` syntax.
    /// The interpolation priority is: `GUNGRAUN_VR_*` variables first, then `--envs` variables,
    /// then the system environment.
    ///
    /// This is useful for running benchmarks in containers or other environments where valgrind is
    /// not available on the host. See the online guide for detailed examples.
    ///
    /// Examples:
    ///   * --valgrind-runner=docker
    ///   * --valgrind-runner=/path/to/wrapper --valgrind-runner-args='--some-flag=${GUNGRAUN_ALLOW_ASLR}'
    #[arg(
        long = "valgrind-runner",
        value_parser = PathBufValueParser::new().try_map(parse_path_resolved),
        num_args = 1,
        verbatim_doc_comment,
        env = "GUNGRAUN_VALGRIND_RUNNER",
        display_order = 500
    )]
    pub valgrind_runner: Option<PathBuf>,

    #[rustfmt::skip]
    /// Additional arguments to pass to the valgrind runner executable
    ///
    /// This option is only effective when `--valgrind-runner` is specified. The arguments are
    /// passed to the runner executable after `--valgrind-runner` and before the valgrind path.
    ///
    /// Environment variable interpolation is supported using the `${VAR}` syntax. Variables are
    /// resolved in this order:
    /// 1. `GUNGRAUN_VR_*` variables set by Gungraun (see `--valgrind-runner` for the list)
    /// 2. Variables specified via `--envs` and `LibraryBenchmarkConfig::envs` or
    ///    `BinaryBenchmarkConfig::envs`
    /// 3. System environment variables
    ///
    /// The interpolation allows passing dynamic values to the runner based on Gungraun's
    /// configuration. For example, `${GUNGRAUN_ALLOW_ASLR}` interpolation is useful for passing
    /// the ASLR setting to container setups.
    ///
    /// Examples:
    ///   * --valgrind-runner=sudo --valgrind-runner-args='--user=foo'
    ///   * --valgrind-runner=wrapper '--valgrind-runner-args=--allow-aslr=${GUNGRAUN_ALLOW_ASLR}'
    #[arg(
        long = "valgrind-runner-args",
        value_parser = parse_raw_args,
        requires = "valgrind_runner",
        required = false,
        num_args = 1,
        action = ArgAction::Append,
        verbatim_doc_comment,
        env = "GUNGRAUN_VALGRIND_RUNNER_ARGS",
        display_order = 500
    )]
    pub valgrind_runner_args: Vec<RawArgs>,

    #[rustfmt::skip]
    /// Override the destination directory path for valgrind runner output files
    ///
    /// This option is only effective when `--valgrind-runner` is specified. By default, valgrind
    /// output files are written to paths under the gungraun home directory or in temporary
    /// directories. This option allows substituting this path with a custom directory.
    ///
    /// When specified, any occurrence of this path prefix in valgrind arguments will be replaced
    /// with the directory path specified by `--valgrind-runner-dest`.
    ///
    /// WARNING: Make sure the directory of this argument exists, is empty and doesn't point to a
    /// directory with important files in it! This directory is managed by Gungraun and Gungraun
    /// might delete **all** files in this directory. More details can be found in the online
    /// guide.
    ///
    /// Examples:
    ///   * `--valgrind-runner-dest=/tmp/results`
    #[arg(
        long = "valgrind-runner-dest",
        num_args = 1,
        requires = "valgrind_runner",
        verbatim_doc_comment,
        env = "GUNGRAUN_VALGRIND_RUNNER_DEST",
        display_order = 500
    )]
    pub valgrind_runner_dest: Option<PathBuf>,

    #[rustfmt::skip]
    /// Override the workspace root path for the valgrind runner
    ///
    /// This option is only effective when `--valgrind-runner` is specified. It allows substituting
    /// the workspace root path prefix in the benchmark executable path and all other valgrind
    /// arguments.
    ///
    /// This can be useful for container setups where the workspace is mounted at a different
    /// location inside the container.
    ///
    /// Examples:
    ///   * `--valgrind-runner-root=/workspace`
    #[arg(
        long = "valgrind-runner-root",
        num_args = 1,
        requires = "valgrind_runner",
        verbatim_doc_comment,
        env = "GUNGRAUN_VALGRIND_RUNNER_ROOT",
        display_order = 500
    )]
    pub valgrind_runner_root: Option<PathBuf>,
}

/// A wrapper type for raw command-line arguments
///
/// Stores a list of raw string arguments without special processing or validation. Used for
/// arguments passed through to external executables without modification, particularly for
/// `--valgrind-runner-args`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawArgs(Vec<String>);

impl BenchmarkFilter {
    /// Return `true` if the filter matches the haystack
    pub fn apply(&self, haystack: &str) -> bool {
        let Self::WildcardPattern(pattern) = self;
        pattern.as_str().dowild_with(haystack, DOWILD_OPTIONS)
    }
}

impl FromStr for BenchmarkFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::WildcardPattern(s.to_owned()))
    }
}

impl NoCapture {
    /// Apply the `NoCapture` option to the [`Command`]
    pub fn apply(
        self,
        command: &mut Command,
        captured_output: Option<&CapturedOutput>,
    ) -> Result<()> {
        match (self, captured_output) {
            (Self::True, Some(captured_output)) => {
                // Both go to the same file, here chosen to be stdout
                command
                    .stdout(captured_output.stdout.try_clone()?)
                    .stderr(captured_output.stdout.try_clone()?);
            }
            (Self::False, Some(captured_output)) => {
                command
                    .stdout(captured_output.stdout.try_clone()?)
                    .stderr(captured_output.stderr.try_clone()?);
            }
            (Self::Stderr, Some(captured_output)) => {
                command
                    .stdout(Stdio::null())
                    .stderr(captured_output.stderr.try_clone()?);
            }
            (Self::Stdout, Some(captured_output)) => {
                command
                    .stdout(captured_output.stdout.try_clone()?)
                    .stderr(Stdio::null());
            }
            (Self::True, None) => {
                command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            }
            (Self::False, None) => {
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
            }
            (Self::Stderr, None) => {
                command.stdout(Stdio::null()).stderr(Stdio::inherit());
            }
            (Self::Stdout, None) => {
                command.stdout(Stdio::inherit()).stderr(Stdio::null());
            }
        }

        Ok(())
    }
}

impl From<TruncateDescription> for Option<usize> {
    fn from(value: TruncateDescription) -> Self {
        match value {
            TruncateDescription::To(to) => Some(to),
            TruncateDescription::None => None,
        }
    }
}

impl RawArgs {
    /// Returns a slice of the underlying argument strings
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// TODO: DOCS
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// TODO: DOCS
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// Convert the `metric` if it is present
//
// Used for example for hard limits to convert u64 values to f64 values if required.
fn convert_metric<T: Display + TypeChecker + Copy>(
    metric_kind: T,
    metric: Option<Metric>,
) -> Result<(T, Option<Metric>), String> {
    if let Some(metric) = metric {
        metric
            .try_convert(metric_kind)
            .ok_or_else(|| {
                format!(
                    "Invalid hard limit for '{metric_kind}': Expected an integer (e.g. '10'). If \
                     you wanted this value to be a soft limit use the '%' suffix (e.g. '4.0%' or \
                     '4%')"
                )
            })
            .map(|(t, m)| (t, Some(m)))
    } else {
        Ok((metric_kind, None))
    }
}

/// Same as `parse_callgrind_limits` but for cachegrind
fn parse_cachegrind_limits(value: &str) -> Result<ToolRegressionConfig, String> {
    let (soft_limits, hard_limits) = parse_limits(value, |key, metric| {
        let metrics = key
            .parse::<CachegrindMetrics>()
            .map_err(|error| error.to_string())?;
        IndexSet::from(metrics)
            .into_iter()
            .map(|metric_kind| convert_metric(metric_kind, metric))
            .collect::<ParsedMetrics<CachegrindMetric>>()
    })?;

    let config = ToolRegressionConfig::Cachegrind(CachegrindRegressionConfig {
        soft_limits: soft_limits.into_iter().collect(),
        hard_limits: hard_limits.into_iter().collect(),
        ..Default::default()
    });

    Ok(config)
}

/// Parse the cachegrind metrics
fn parse_cachegrind_metrics(value: &str) -> Result<IndexSet<CachegrindMetric>, String> {
    parse_tool_metrics(value, |item| {
        item.parse::<CachegrindMetrics>()
            .map(IndexSet::from)
            .map_err(|error| error.to_string())
    })
}

/// Parse the callgrind limits from the command-line
///
/// This method (and the other `parse_dhat_limits`, ...) parses soft and hard limits in one go. The
/// format is described in the --help message above in [`CommandLineArgs`].
///
/// In order to avoid back and forth conversions between `api::ToolRegressionConfig` and
/// `tool::ToolRegressionConfig` we parse the `tool::ToolRegressionConfig` directly.
fn parse_callgrind_limits(value: &str) -> Result<ToolRegressionConfig, String> {
    let (soft_limits, hard_limits) = parse_limits(value, |key, metric| {
        let metrics = key
            .parse::<CallgrindMetrics>()
            .map_err(|error| error.to_string())?;
        IndexSet::from(metrics)
            .into_iter()
            .map(|event_kind| convert_metric(event_kind, metric))
            .collect::<ParsedMetrics<EventKind>>()
    })?;

    let config = ToolRegressionConfig::Callgrind(CallgrindRegressionConfig {
        soft_limits: soft_limits.into_iter().collect(),
        hard_limits: hard_limits.into_iter().collect(),
        ..Default::default()
    });

    Ok(config)
}

/// Parse the callgrind metrics
fn parse_callgrind_metrics(value: &str) -> Result<IndexSet<EventKind>, String> {
    parse_tool_metrics(value, |item| {
        item.parse::<CallgrindMetrics>()
            .map(IndexSet::from)
            .map_err(|error| error.to_string())
    })
}

/// Same as `parse_callgrind_limits` but for dhat
fn parse_dhat_limits(value: &str) -> Result<ToolRegressionConfig, String> {
    let (soft_limits, hard_limits) = parse_limits(value, |key, metric| {
        let metrics = key
            .parse::<DhatMetrics>()
            .map_err(|error| error.to_string())?;
        IndexSet::from(metrics)
            .into_iter()
            .map(|metric_kind| convert_metric(metric_kind, metric))
            .collect::<ParsedMetrics<DhatMetric>>()
    })?;

    let config = ToolRegressionConfig::Dhat(DhatRegressionConfig {
        soft_limits: soft_limits.into_iter().collect(),
        hard_limits: hard_limits.into_iter().collect(),
        ..Default::default()
    });

    Ok(config)
}

/// Parse the DHAT metrics
fn parse_dhat_metrics(value: &str) -> Result<IndexSet<DhatMetric>, String> {
    parse_tool_metrics(value, |item| {
        item.parse::<DhatMetrics>()
            .map(IndexSet::from)
            .map_err(|error| error.to_string())
    })
}

/// Parse the DRD metrics as error metrics
fn parse_drd_metrics(value: &str) -> Result<IndexSet<ErrorMetric>, String> {
    parse_tool_metrics(value, parse_error_metrics)
}

/// Parse environment variable `key=value` pairs and resolve standalone keys
fn parse_envs(value: &str) -> Result<Vec<(OsString, OsString)>, String> {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
        .or_else(|| trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
        .unwrap_or(trimmed);

    let splits = shlex::split(trimmed)
        .ok_or_else(|| format!("Failed splitting '{value}' for POSIX shell environment"))?;

    let mut result = vec![];
    for split in splits {
        if let Some((key, equals_value)) = split.split_once('=') {
            if key.is_empty() {
                return Err(format!("Empty key for value: '{equals_value}'"));
            }

            result.push((OsString::from(key), OsString::from(equals_value)));
        } else if let Some(env_value) = std::env::var_os(&split) {
            result.push((OsString::from(split), env_value));
        } else {
            // do nothing
        }
    }

    Ok(result)
}

fn parse_error_metrics(item: &str) -> Result<IndexSet<ErrorMetric>, String> {
    if let Some(prefix) = item.strip_prefix('@') {
        if prefix == "all" {
            Ok(ErrorMetric::iter().fold(IndexSet::new(), |mut acc, elem| {
                acc.insert(elem);
                acc
            }))
        } else {
            Err(format!("Invalid error metric group: '{item}"))
        }
    } else {
        let metric = item
            .parse::<ErrorMetric>()
            .map_err(|error| error.to_string())?;
        Ok(indexset! { metric })
    }
}

/// Parse the helgrind metrics as error metrics
fn parse_helgrind_metrics(value: &str) -> Result<IndexSet<ErrorMetric>, String> {
    parse_tool_metrics(value, parse_error_metrics)
}

fn parse_limits<T: Eq + Hash>(
    value: &str,
    parse_metrics: fn(&str, Option<Metric>) -> ParsedMetrics<T>,
) -> Result<Limits<T>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("No limits found: At least one limit must be present".to_owned());
    }

    let mut soft_limits = IndexMap::new();
    let mut hard_limits = IndexMap::new();

    for item in value.split(',') {
        let item = item.trim();

        if let Some((key, value)) = item.split_once('=') {
            let (key, value) = (key.trim(), value.trim());
            for split in value.split('|') {
                let split = split.trim();

                if let Some(prefix) = split.strip_suffix('%') {
                    let pct = prefix.parse::<f64>().map_err(|error| -> String {
                        format!("Invalid soft limit for '{key}': {error}")
                    })?;
                    let metric_kinds = parse_metrics(key, None)?;
                    for (metric_kind, _) in metric_kinds {
                        soft_limits.insert(metric_kind, pct);
                    }
                } else {
                    let metric = split.parse::<Metric>().map_err(|error| -> String {
                        format!("Invalid hard limit for '{key}': {error}")
                    })?;
                    let metric_kinds = parse_metrics(key, Some(metric))?;
                    for (metric_kind, new_metric) in metric_kinds {
                        if let Some(new_metric) = new_metric {
                            hard_limits.insert(metric_kind, new_metric);
                        } else {
                            hard_limits.insert(metric_kind, metric);
                        }
                    }
                }
            }
        } else {
            return Err(format!("Invalid format of key=value pair: '{item}'"));
        }
    }

    Ok((soft_limits, hard_limits))
}

/// Parse the memcheck metrics as error metrics
fn parse_memcheck_metrics(value: &str) -> Result<IndexSet<ErrorMetric>, String> {
    parse_tool_metrics(value, parse_error_metrics)
}

/// Parse --nocapture
fn parse_nocapture(value: &str) -> Result<NoCapture, String> {
    // Taken from clap source code
    const TRUE_LITERALS: [&str; 6] = ["y", "yes", "t", "true", "on", "1"];
    const FALSE_LITERALS: [&str; 6] = ["n", "no", "f", "false", "off", "0"];

    let lowercase = value.to_lowercase();

    if TRUE_LITERALS.contains(&lowercase.as_str()) {
        Ok(NoCapture::True)
    } else if FALSE_LITERALS.contains(&lowercase.as_str()) {
        Ok(NoCapture::False)
    } else if lowercase == "stdout" {
        Ok(NoCapture::Stdout)
    } else if lowercase == "stderr" {
        Ok(NoCapture::Stderr)
    } else {
        Err(format!("Invalid value: {value}"))
    }
}

/// Parse --parallel
fn parse_parallel(value: &str) -> Result<usize, String> {
    let lowercase = value.to_lowercase();

    if lowercase == "auto" {
        Ok(num_cpus::get())
    } else if let Ok(num) = lowercase.parse::<usize>() {
        if num > 0 {
            Ok(num)
        } else {
            Err(format!("Value must be greater than 0 but was '{value}'"))
        }
    } else {
        Err(format!("Invalid value: {value}"))
    }
}

fn parse_path_resolved(value: PathBuf) -> Result<PathBuf, String> {
    util::resolve_binary_path(value, None).map_err(|error| error.to_string())
}

/// This function parses a space separated list of raw argument strings into [`RawArgs`]
fn parse_raw_args(value: &str) -> Result<RawArgs, String> {
    let value = if value.is_empty() {
        return Err(String::from("Empty arguments"));
    } else if value.len() >= 2 {
        match (&value.as_bytes()[0], &value.as_bytes()[value.len() - 1]) {
            (b'\'', b'\'') | (b'"', b'"') => &value[1..value.len() - 1],
            _ => value,
        }
    } else {
        value
    };

    shlex::split(value)
        .ok_or_else(|| "Failed to split args".to_owned())
        .map(RawArgs)
}

/// This function parses a space separated list of raw argument strings into
/// [`crate::api::RawToolArgs`]
fn parse_tool_args(value: &str) -> Result<RawToolArgs, String> {
    parse_raw_args(value).map(|r| RawToolArgs::new(r.0))
}

/// Utility function to parse the --callgrind-metrics, ...
fn parse_tool_metrics<T: Eq + Hash>(
    value: &str,
    parse_metrics: fn(&str) -> Result<IndexSet<T>, String>,
) -> Result<IndexSet<T>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("No metric found: At least one metric must be present".to_owned());
    }

    let mut format = IndexSet::new();

    for item in value.split(',') {
        let item = item.trim();
        let metrics = parse_metrics(item)?;
        format.extend(metrics);
    }

    Ok(format)
}

fn parse_truncate_description(value: &str) -> Result<TruncateDescription, String> {
    // Almost the same as the BoolishValueParser but without `1` and `0` which are interpreted as
    // values. The FALSE_LITERALS also contain `none` as special value.
    const TRUE_LITERALS: [&str; 5] = ["y", "yes", "t", "true", "on"];
    const FALSE_LITERALS: [&str; 6] = ["n", "no", "none", "f", "false", "off"];

    let lowercase = value.to_lowercase();

    if TRUE_LITERALS.contains(&lowercase.as_str()) {
        Ok(TruncateDescription::To(50))
    } else if FALSE_LITERALS.contains(&lowercase.as_str()) {
        Ok(TruncateDescription::None)
    } else if let Ok(parsed) = lowercase.parse::<usize>() {
        Ok(TruncateDescription::To(parsed))
    } else {
        Err(format!("Invalid value: {value}"))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    use rstest::rstest;
    use tempfile::{tempdir, NamedTempFile};

    use super::*;
    use crate::api::EventKind::*;
    use crate::api::RawToolArgs;

    #[rstest]
    #[case::single_key_value("--some=yes", &["--some=yes"])]
    #[case::two_key_value("--some=yes --other=no", &["--some=yes", "--other=no"])]
    #[case::single_escaped("--some='yes and no'", &["--some=yes and no"])]
    #[case::double_escaped("--some='\"yes and no\"'", &["--some=\"yes and no\""])]
    #[case::multiple_escaped(
        "--some='yes and no' --other='no and yes'",
        &["--some=yes and no", "--other=no and yes"]
    )]
    fn test_parse_tool_args(#[case] value: &str, #[case] expected: &[&str]) {
        let actual = parse_tool_args(value).unwrap();
        assert_eq!(actual, RawToolArgs::from_iter(expected));
    }

    #[test]
    fn test_parse_tool_args_when_empty_then_error() {
        parse_tool_args("").unwrap_err();
    }

    #[rstest]
    #[case::single_soft("Ir=10%", vec![(Ir, 10f64)], vec![])]
    #[case::single_hard("Ir=20", vec![], vec![(Ir, 20.into())])]
    #[case::soft_and_hard("Ir=20|10%", vec![(Ir, 10f64)], vec![(Ir, 20.into())])]
    #[case::soft_and_hard_separated("Ir=20, Ir=10%", vec![(Ir, 10f64)], vec![(Ir, 20.into())])]
    #[case::soft_overwrite("Ir=20%, Ir=10%", vec![(Ir, 10f64)], vec![])]
    #[case::hard_overwrite("Ir=20, Ir=10", vec![], vec![(Ir, 10.into())])]
    #[case::group_wb_soft("@wb=10%", vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)], vec![])]
    #[case::group_writeback_soft(
        "@writeback=10%",
        vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)],
        vec![]
    )]
    #[case::group_writebackbehaviour_soft(
        "@writebackbehaviour=10%",
        vec![(ILdmr, 10f64), (DLdmr, 10f64), (DLdmw, 10f64)],
        vec![]
    )]
    #[case::group_hr_hard_int(
        "@hr=10",
        vec![],
        vec![(L1HitRate, 10f64.into()), (LLHitRate, 10f64.into()), (RamHitRate, 10f64.into())]
    )]
    #[case::group_hr_hard_float(
        "@hr=10.0",
        vec![],
        vec![(L1HitRate, 10f64.into()), (LLHitRate, 10f64.into()), (RamHitRate, 10f64.into())]
    )]
    #[case::case_insensitive(
        "EstIMATedCycles=10%",
        vec![(EstimatedCycles, 10f64)],
        vec![]
    )]
    #[case::multiple_soft(
        "Ir=10%,EstimatedCycles=5%",
        vec![(Ir, 10f64), (EstimatedCycles, 5f64)],
        vec![]
    )]
    #[case::multiple_hard(
        "Ir=20,EstimatedCycles=50",
        vec![],
        vec![(Ir, 20.into()), (EstimatedCycles, 50.into())]
    )]
    #[case::with_whitespace(
        "Ir= 10% , EstimatedCycles = 5%",
        vec![(Ir, 10f64), (EstimatedCycles, 5f64)],
        vec![]
    )]
    fn test_parse_callgrind_limits(
        #[case] regression_var: &str,
        #[case] expected_soft_limits: Vec<(EventKind, f64)>,
        #[case] expected_hard_limits: Vec<(EventKind, Metric)>,
    ) {
        if let ToolRegressionConfig::Callgrind(CallgrindRegressionConfig {
            soft_limits,
            hard_limits,
            ..
        }) = parse_callgrind_limits(regression_var).unwrap()
        {
            assert_eq!(soft_limits, expected_soft_limits);
            assert_eq!(hard_limits, expected_hard_limits);
        } else {
            panic!("Wrong regression config");
        }
    }

    #[rstest]
    #[case::regression_wrong_format_of_key_value_pair(
        "Ir:10",
        "Invalid format of key=value pair: 'Ir:10'"
    )]
    #[case::regression_unknown_event_kind("WRONG=10", "Unknown event kind: 'WRONG'")]
    #[case::float_instead_of_integer(
        "Ir=10.0",
        "Invalid hard limit for 'Instructions': Expected an integer (e.g. '10'). If you wanted \
         this value to be a soft limit use the '%' suffix (e.g. '4.0%' or '4%')"
    )]
    #[case::regression_invalid_percentage(
        "Ir=10.0.0",
        "Invalid hard limit for 'Ir': Invalid metric: invalid float literal"
    )]
    #[case::invalid_soft_limit("Ir=abc%", "Invalid soft limit for 'Ir': invalid float literal")]
    #[case::regression_empty_limits("", "No limits found: At least one limit must be present")]
    fn test_parse_callgrind_limits_then_error(
        #[case] regression_var: &str,
        #[case] expected_reason: &str,
    ) {
        assert_eq!(
            &parse_callgrind_limits(regression_var).unwrap_err(),
            expected_reason,
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_callgrind_args_env() {
        let test_arg = "--just-testing=yes";
        std::env::set_var("GUNGRAUN_CALLGRIND_ARGS", test_arg);
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.callgrind_args,
            Some(RawToolArgs::new(vec![test_arg.to_owned()]))
        );
    }

    #[rstest]
    #[case::without_flag("--callgrind-args=foo", &["--foo"])]
    #[case::with_flag("--callgrind-args=--foo", &["--foo"])]
    #[case::without_flag_and_quotes("--callgrind-args='foo'", &["--foo"])]
    #[case::with_flag_and_quotes("--callgrind-args='--foo'", &["--foo"])]
    #[case::with_equals("--callgrind-args=--foo=bar", &["--foo=bar"])]
    #[case::two_flags("--callgrind-args='--foo=bar --bar=baz'", &["--foo=bar", "--bar=baz"])]
    #[case::two_without_flags("--callgrind-args='foo=bar bar=baz'", &["--foo=bar", "--bar=baz"])]
    fn test_callgrind_args_not_env(#[case] input: &str, #[case] expected: &[&str]) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(
            result.callgrind_args,
            Some(RawToolArgs::new(expected.iter().map(ToOwned::to_owned)))
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_callgrind_args_cli_takes_precedence_over_env() {
        let test_arg_yes = "--just-testing=yes";
        let test_arg_no = "--just-testing=no";
        std::env::set_var("GUNGRAUN_CALLGRIND_ARGS", test_arg_yes);
        let result = CommandLineArgs::parse_from([format!("--callgrind-args={test_arg_no}")]);
        assert_eq!(
            result.callgrind_args,
            Some(RawToolArgs::new(vec![test_arg_no.to_owned()]))
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_save_summary_env() {
        std::env::set_var("GUNGRAUN_SAVE_SUMMARY", "json");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.save_summary, Some(SummaryFormat::Json));
    }

    #[rstest]
    #[case::default("", SummaryFormat::Json)]
    #[case::json("json", SummaryFormat::Json)]
    #[case::pretty_json("pretty-json", SummaryFormat::PrettyJson)]
    fn test_save_summary_cli(#[case] value: &str, #[case] expected: SummaryFormat) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--save-summary".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--save-summary={value}")])
        };
        assert_eq!(result.save_summary, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_allow_aslr_env() {
        std::env::set_var("GUNGRAUN_ALLOW_ASLR", "yes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.allow_aslr, Some(true));
    }

    #[rstest]
    #[case::default("", true)]
    #[case::yes("yes", true)]
    #[case::no("no", false)]
    fn test_allow_aslr_cli(#[case] value: &str, #[case] expected: bool) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--allow-aslr".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--allow-aslr={value}")])
        };
        assert_eq!(result.allow_aslr, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_separate_targets_env() {
        std::env::set_var("GUNGRAUN_SEPARATE_TARGETS", "yes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert!(result.separate_targets);
    }

    #[rstest]
    #[case::default("", true)]
    #[case::yes("yes", true)]
    #[case::no("no", false)]
    fn test_separate_targets_cli(#[case] value: &str, #[case] expected: bool) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--separate-targets".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--separate-targets={value}")])
        };
        assert_eq!(result.separate_targets, expected);
    }

    #[test]
    #[serial_test::serial]
    fn test_home_env() {
        std::env::set_var("GUNGRAUN_HOME", "/tmp/my_gungraun_home");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.home, Some(PathBuf::from("/tmp/my_gungraun_home")));
    }

    #[test]
    fn test_home_cli() {
        let result = CommandLineArgs::parse_from(["--home=/test_me".to_owned()]);
        assert_eq!(result.home, Some(PathBuf::from("/test_me")));
    }

    #[test]
    fn test_home_cli_when_no_value_then_error() {
        let result = CommandLineArgs::try_parse_from(["--home=".to_owned()]);
        result.unwrap_err();
    }

    #[rstest]
    #[case::default("", NoCapture::True)]
    #[case::yes("true", NoCapture::True)]
    #[case::no("false", NoCapture::False)]
    #[case::stdout("stdout", NoCapture::Stdout)]
    #[case::stderr("stderr", NoCapture::Stderr)]
    fn test_nocapture_cli(#[case] value: &str, #[case] expected: NoCapture) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--nocapture".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--nocapture={value}")])
        };
        assert_eq!(result.nocapture, expected);
    }

    #[test]
    #[serial_test::serial]
    fn test_nocapture_env() {
        std::env::set_var("GUNGRAUN_NOCAPTURE", "true");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.nocapture, NoCapture::True);
    }

    #[rstest]
    #[case::single("drd", &[ValgrindTool::DRD])]
    #[case::two("drd,callgrind", &[ValgrindTool::DRD, ValgrindTool::Callgrind])]
    fn test_tools_cli(#[case] tools: &str, #[case] expected: &[ValgrindTool]) {
        let actual = CommandLineArgs::parse_from([format!("--tools={tools}")]);
        assert_eq!(actual.tools, expected);
    }

    #[rstest]
    #[case::y("y", true)]
    #[case::yes("yes", true)]
    #[case::t("t", true)]
    #[case::true_value("true", true)]
    #[case::on("on", true)]
    #[case::one("1", true)]
    #[case::n("n", false)]
    #[case::no("no", false)]
    #[case::f("f", false)]
    #[case::false_value("false", false)]
    #[case::off("off", false)]
    #[case::zero("0", false)]
    fn test_boolish(#[case] value: &str, #[case] expected: bool) {
        let result = CommandLineArgs::parse_from(&[format!("--allow-aslr={value}")]);
        assert_eq!(result.allow_aslr, Some(expected));
    }

    #[rstest]
    #[case::include_ignored("--include-ignored", "")]
    #[case::ignored("--ignored", "")]
    #[case::force_run_in_process("--force-run-in-process", "")]
    #[case::exclude_should_panic("--exclude-should-panic", "")]
    #[case::test("--test", "")]
    #[case::bench("--bench", "")]
    #[case::logfile_without_arg("--logfile", "")]
    #[case::logfile_with_arg("--logfile", "/some/path")]
    #[case::test_threads("--test-threads", "")]
    #[case::skip_without_arg("--skip", "")]
    #[case::skip_with_arg("--skip", "some::test")]
    #[case::quiet_short("-q", "")]
    #[case::quiet_long("--quiet", "")]
    #[case::exact("--exact", "")]
    #[case::color_without_arg("--color", "")]
    #[case::color_with_arg("--color", "auto")]
    #[case::format_without_arg("--format", "")]
    #[case::format_with_arg("--format", "terse")]
    #[case::show_output("--show-output", "")]
    #[case::z_without_arg("-Z", "")]
    #[case::z_with_arg("-Z", "unstable-options")]
    #[case::report_time("--report-time", "")]
    #[case::ensure_time("--ensure-time", "")]
    #[case::shuffle("--shuffle", "")]
    #[case::shuffle_seed_without_arg("--shuffle-seed", "")]
    #[case::shuffle_seed_with_arg("--shuffle-seed", "123")]
    fn test_when_libtest_arg_then_no_exit_with_error(#[case] arg: &str, #[case] value: &str) {
        let result = if value.is_empty() {
            CommandLineArgs::try_parse_from([arg])
        } else {
            CommandLineArgs::try_parse_from(&[format!("{arg}={value}")])
        };

        result.unwrap();
    }

    #[rstest]
    #[case::one("ir", indexset!{ Ir })]
    #[case::one_with_spaces("  ir ", indexset!{ Ir })]
    #[case::two("ir,i1mr", indexset!{ Ir, I1mr })]
    #[case::two_with_spaces("ir,   i1mr", indexset!{ Ir, I1mr })]
    #[case::group("@writebackbehaviour", indexset!{ ILdmr, DLdmr, DLdmw })]
    #[case::group_abbreviation("@wb", indexset!{ ILdmr, DLdmr, DLdmw })]
    #[case::group_and_single_then_no_change("@wb,ildmr", indexset!{ ILdmr, DLdmr, DLdmw })]
    #[case::single_and_group_then_overwrite("dldmw,@wb", indexset!{ DLdmw, ILdmr, DLdmr })]
    #[case::all("@all", CallgrindMetrics::All.into())]
    fn test_parse_callgrind_metrics(#[case] input: &str, #[case] expected: IndexSet<EventKind>) {
        assert_eq!(parse_callgrind_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::empty("")]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    #[case::wrong_delimiter("ir;dr")]
    fn test_parse_callgrind_metrics_then_error(#[case] input: &str) {
        parse_callgrind_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_callgrind_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--callgrind-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_callgrind_metrics_when_env() {
        std::env::set_var("GUNGRAUN_CALLGRIND_METRICS", "ir");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.callgrind_metrics,
            Some(IndexSet::from([EventKind::Ir]))
        );
    }

    // Just test the very basics. The details are tested in `test_parse_callgrind_metrics`
    #[rstest]
    #[case::one("ir", indexset!{ CachegrindMetric::Ir })]
    #[case::all("@all", CachegrindMetrics::All.into())]
    fn test_parse_cachegrind_metrics(
        #[case] input: &str,
        #[case] expected: IndexSet<CachegrindMetric>,
    ) {
        assert_eq!(parse_cachegrind_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_cachegrind_metrics_then_error(#[case] input: &str) {
        parse_cachegrind_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_cachegrind_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--cachegrind-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_cachegrind_metrics_when_env() {
        std::env::set_var("GUNGRAUN_CACHEGRIND_METRICS", "ir");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.cachegrind_metrics,
            Some(IndexSet::from([CachegrindMetric::Ir]))
        );
    }

    #[rstest]
    #[case::one("totalbytes", indexset!{ DhatMetric::TotalBytes })]
    #[case::all("@all", DhatMetrics::All.into())]
    fn test_parse_dhat_metrics(#[case] input: &str, #[case] expected: IndexSet<DhatMetric>) {
        assert_eq!(parse_dhat_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_dhat_metrics_then_error(#[case] input: &str) {
        parse_dhat_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_dhat_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--dhat-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_dhat_metrics_when_env() {
        std::env::set_var("GUNGRAUN_DHAT_METRICS", "totalbytes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.dhat_metrics,
            Some(IndexSet::from([DhatMetric::TotalBytes]))
        );
    }

    #[rstest]
    #[case::one("errors", indexset!{ ErrorMetric::Errors })]
    #[case::all("@all", indexset! {
        ErrorMetric::Errors,
        ErrorMetric::Contexts,
        ErrorMetric::SuppressedErrors,
        ErrorMetric::SuppressedContexts
    })]
    fn test_parse_drd_metrics(#[case] input: &str, #[case] expected: IndexSet<ErrorMetric>) {
        assert_eq!(parse_drd_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_drd_metrics_then_error(#[case] input: &str) {
        parse_drd_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_drd_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--drd-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_drd_metrics_when_env() {
        std::env::set_var("GUNGRAUN_DRD_METRICS", "errors");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.drd_metrics,
            Some(IndexSet::from([ErrorMetric::Errors]))
        );
    }

    #[rstest]
    #[case::one("errors", indexset!{ ErrorMetric::Errors })]
    #[case::all("@all", indexset! {
        ErrorMetric::Errors,
        ErrorMetric::Contexts,
        ErrorMetric::SuppressedErrors,
        ErrorMetric::SuppressedContexts
    })]
    fn test_parse_memcheck_metrics(#[case] input: &str, #[case] expected: IndexSet<ErrorMetric>) {
        assert_eq!(parse_memcheck_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_memcheck_metrics_then_error(#[case] input: &str) {
        parse_memcheck_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_memcheck_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--memcheck-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_memcheck_metrics_when_env() {
        std::env::set_var("GUNGRAUN_MEMCHECK_METRICS", "errors");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.memcheck_metrics,
            Some(IndexSet::from([ErrorMetric::Errors]))
        );
    }

    #[rstest]
    #[case::one("errors", indexset!{ ErrorMetric::Errors })]
    #[case::all("@all", indexset! {
        ErrorMetric::Errors,
        ErrorMetric::Contexts,
        ErrorMetric::SuppressedErrors,
        ErrorMetric::SuppressedContexts
    })]
    fn test_parse_helgrind_metrics(#[case] input: &str, #[case] expected: IndexSet<ErrorMetric>) {
        assert_eq!(parse_helgrind_metrics(input).unwrap(), expected);
    }

    #[rstest]
    #[case::event_kind_does_not_exist("doesnotexist")]
    #[case::group_does_not_exist("@doesnotexist")]
    fn test_parse_helgrind_metrics_then_error(#[case] input: &str) {
        parse_helgrind_metrics(input).unwrap_err();
    }

    #[test]
    fn test_arg_helgrind_metrics_when_empty_then_error() {
        CommandLineArgs::try_parse_from(["--helgrind-metrics"]).unwrap_err();
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_helgrind_metrics_when_env() {
        std::env::set_var("GUNGRAUN_HELGRIND_METRICS", "errors");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.helgrind_metrics,
            Some(IndexSet::from([ErrorMetric::Errors]))
        );
    }

    #[rstest]
    #[case::default("--tolerance", f64::from_bits(0.000_01f64.to_bits() - 1))]
    #[case::some_value("--tolerance=1.0", 1.0)]
    fn test_arg_tolerance(#[case] input: &str, #[case] expected: f64) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.tolerance, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_tolerance_when_env() {
        std::env::set_var("GUNGRAUN_TOLERANCE", "2.0");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.tolerance, Some(2.0));
    }

    #[rstest]
    #[case::when_no_equals("--show-intermediate", true)]
    #[case::when_true("--show-intermediate=true", true)]
    #[case::when_false("--show-intermediate=false", false)]
    fn test_arg_show_intermediate(#[case] input: &str, #[case] expected: bool) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.show_intermediate, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_show_intermediate_when_env() {
        std::env::set_var("GUNGRAUN_SHOW_INTERMEDIATE", "yes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.show_intermediate, Some(true));
    }

    #[rstest]
    #[case::when_no_equals("--show-grid", true)]
    #[case::when_true("--show-grid=true", true)]
    #[case::when_false("--show-grid=false", false)]
    fn test_arg_show_grid(#[case] input: &str, #[case] expected: bool) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.show_grid, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_show_grid_when_env() {
        std::env::set_var("GUNGRAUN_SHOW_GRID", "yes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.show_grid, Some(true));
    }

    #[rstest]
    #[case::missing_value("--truncate-description", TruncateDescription::To(50))]
    #[case::some_value("--truncate-description=20", TruncateDescription::To(20))]
    #[case::when_false("--truncate-description=false", TruncateDescription::None)]
    #[case::when_no("--truncate-description=no", TruncateDescription::None)]
    fn test_arg_truncate_description(#[case] input: &str, #[case] expected: TruncateDescription) {
        let result = CommandLineArgs::try_parse_from([input]).unwrap();
        assert_eq!(result.truncate_description, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_arg_truncate_description_when_env() {
        std::env::set_var("GUNGRAUN_TRUNCATE_DESCRIPTION", "no");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.truncate_description, Some(TruncateDescription::None));
    }

    #[test]
    fn test_arg_valgrind_runner() {
        let file = tempfile::Builder::new()
            .permissions(Permissions::from_mode(0o755))
            .tempfile()
            .unwrap();
        let result = CommandLineArgs::try_parse_from([format!(
            "--valgrind-runner={}",
            file.path().display()
        )])
        .unwrap();

        assert_eq!(result.valgrind_runner, Some(file.path().to_path_buf()));
    }

    #[test]
    fn test_arg_valgrind_runner_when_directory_then_error() {
        let dir = tempdir().unwrap();
        let result = CommandLineArgs::try_parse_from([format!(
            "--valgrind-runner='{}'",
            dir.path().display()
        )]);
        result.unwrap_err();
    }

    #[test]
    fn test_arg_valgrind_runner_when_not_executable_then_error() {
        let file = NamedTempFile::new().unwrap();
        let result = CommandLineArgs::try_parse_from([format!(
            "--valgrind-runner={}",
            file.path().display()
        )]);
        result.unwrap_err();
    }

    #[rstest]
    #[case::positional_one(&["--valgrind-runner-args=foo"], &["foo"])]
    #[case::positional_one_with_quotes(&["--valgrind-runner-args='foo'"], &["foo"])]
    #[case::flag_one(&["--valgrind-runner-args=--foo"], &["--foo"])]
    #[case::flag_one_with_quotes(&["--valgrind-runner-args='--foo'"], &["--foo"])]
    #[case::flag_one_with_equals(&["--valgrind-runner-args=--foo=some"], &["--foo=some"])]
    #[case::flag_two(&["--valgrind-runner-args='--foo --bar'"], &["--foo", "--bar"])]
    fn test_valgrind_runner_args(#[case] input: &[&str], #[case] expected: &[&str]) {
        let result = CommandLineArgs::try_parse_from(
            input
                .iter()
                .chain(std::iter::once(&"--valgrind-runner=/bin/cat")),
        )
        .map_err(|e| e.to_string())
        .unwrap();
        assert_eq!(
            result.valgrind_runner_args,
            vec![RawArgs(expected.iter().map(ToString::to_string).collect())]
        );
    }

    #[test]
    fn test_valgrind_runner_args_when_twice() {
        let result = CommandLineArgs::try_parse_from([
            "--valgrind-runner-args=--foo",
            "--valgrind-runner-args=--bar",
            "--valgrind-runner=/bin/cat",
        ])
        .unwrap();
        assert_eq!(
            result.valgrind_runner_args,
            vec![
                RawArgs(vec!["--foo".to_owned()]),
                RawArgs(vec!["--bar".to_owned()])
            ]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_env_clear_default() {
        std::env::remove_var("GUNGRAUN_ENV_CLEAR");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.env_clear, None);
    }

    #[test]
    #[serial_test::serial]
    fn test_env_clear_env() {
        std::env::set_var("GUNGRAUN_ENV_CLEAR", "yes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(result.env_clear, Some(true));
        std::env::remove_var("GUNGRAUN_ENV_CLEAR");
    }

    #[rstest]
    #[case::yes("yes", true)]
    #[case::no("no", false)]
    #[case::true_val("true", true)]
    #[case::false_val("false", false)]
    #[case::on("on", true)]
    #[case::off("off", false)]
    #[case::one("1", true)]
    #[case::zero("0", false)]
    #[case::default("", true)]
    fn test_env_clear_cli(#[case] value: &str, #[case] expected: bool) {
        let result = if value.is_empty() {
            CommandLineArgs::parse_from(["--env-clear".to_owned()])
        } else {
            CommandLineArgs::parse_from([format!("--env-clear={value}")])
        };
        assert_eq!(result.env_clear, Some(expected));
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_all_missing_vars() {
        let result =
            CommandLineArgs::try_parse_from(["--envs='NONEXISTENT1 NONEXISTENT2'"]).unwrap();

        assert_eq!(result.envs.len(), 1);
        assert_eq!(result.envs[0], vec![]);
    }

    #[test]
    fn test_envs_arg_empty_string() {
        let result = CommandLineArgs::try_parse_from(["--envs=''"]).unwrap();
        assert_eq!(result.envs.len(), 1);
        assert_eq!(result.envs[0], vec![]);
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_from_config_env() {
        std::env::set_var("GUNGRAUN_ENVS", "FROM_CONFIG=yes");
        let result = CommandLineArgs::parse_from::<[_; 0], &str>([]);
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("FROM_CONFIG"), OsString::from("yes"))]
        );
        std::env::remove_var("GUNGRAUN_ENVS");
    }

    #[test]
    fn test_envs_arg_missing_env_var() {
        let result = CommandLineArgs::try_parse_from(["--envs=NONEXISTENT_VAR_789"]).unwrap();
        assert_eq!(result.envs[0], vec![]);
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_mixed_resolution() {
        std::env::set_var("MIXED_TEST_VAR", "from_env");
        let result =
            CommandLineArgs::try_parse_from(["--envs='KEY=val MIXED_TEST_VAR OTHER=set'"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![
                (OsString::from("KEY"), OsString::from("val")),
                (OsString::from("MIXED_TEST_VAR"), OsString::from("from_env")),
                (OsString::from("OTHER"), OsString::from("set")),
            ]
        );
        std::env::remove_var("MIXED_TEST_VAR");
    }

    #[test]
    fn test_envs_arg_multiple_delimited() {
        let result = CommandLineArgs::try_parse_from(["--envs='A=1 B=2 C=3'"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![
                (OsString::from("A"), OsString::from("1")),
                (OsString::from("B"), OsString::from("2")),
                (OsString::from("C"), OsString::from("3")),
            ]
        );
    }

    #[test]
    fn test_envs_arg_multiple_invocations() {
        let result = CommandLineArgs::try_parse_from(["--envs=A=1", "--envs=B=2"]).unwrap();
        assert_eq!(
            result.envs,
            vec![
                vec![(OsString::from("A"), OsString::from("1"))],
                vec![(OsString::from("B"), OsString::from("2"))],
            ]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_partial_resolve() {
        std::env::set_var("PARTIAL_EXISTS", "yes");
        let result =
            CommandLineArgs::try_parse_from(["--envs='PARTIAL_EXISTS NONEXISTENT_XYZ'"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("PARTIAL_EXISTS"), OsString::from("yes"))]
        );
        std::env::remove_var("PARTIAL_EXISTS");
    }

    #[test]
    fn test_envs_arg_path_with_colons() {
        let result = CommandLineArgs::try_parse_from(["--envs=PATH=/usr/bin:/bin"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("PATH"), OsString::from("/usr/bin:/bin"))]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_envs_arg_resolve_from_env() {
        std::env::set_var("RESOLVE_ME_VAR", "env_value");
        let result = CommandLineArgs::try_parse_from(["--envs=RESOLVE_ME_VAR"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(
                OsString::from("RESOLVE_ME_VAR"),
                OsString::from("env_value")
            )]
        );
        std::env::remove_var("RESOLVE_ME_VAR");
    }

    #[rstest]
    #[case::simple(
        &["--envs=KEY=value"],
        vec![(OsString::from("KEY"), OsString::from("value"))]
    )]
    #[case::with_equals_in_value(
        &["--envs=URL=http://example.com"],
        vec![(OsString::from("URL"), OsString::from("http://example.com"))]
    )]
    #[case::empty_value(
        &["--envs=EMPTY="],
        vec![(OsString::from("EMPTY"), OsString::from(""))]
    )]
    #[case::multiple_equals(
        &["--envs=A=B=C"],
        vec![(OsString::from("A"), OsString::from("B=C"))]
    )]
    #[case::with_single_quotes(
        &["--envs='A=foo bar'"],
        vec![(OsString::from("A"), OsString::from("foo"))]
    )]
    #[case::with_single_quotes_value(
        &["--envs=A='foo bar'"],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::with_single_quotes_all(
        &["--envs='A='foo bar''"],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::with_double_quotes(
        &["--envs=\"A=foo bar\""],
        vec![(OsString::from("A"), OsString::from("foo"))]
    )]
    #[case::with_double_quotes_value(
        &["--envs=A=\"foo bar\""],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::with_double_quotes_all(
        &["--envs=\"A=\"foo bar\"\""],
        vec![(OsString::from("A"), OsString::from("foo bar"))]
    )]
    #[case::multiple_with_quotes(
        &["--envs=\"A='foo bar' B=baz\""],
        vec![
            (OsString::from("A"), OsString::from("foo bar")),
            (OsString::from("B"), OsString::from("baz"))
        ]
    )]
    fn test_envs_arg_single(#[case] args: &[&str], #[case] expected: Vec<(OsString, OsString)>) {
        let result = CommandLineArgs::try_parse_from(args).unwrap();
        let expected: Vec<(OsString, OsString)> = expected.into_iter().collect();
        assert_eq!(result.envs[0], expected);
    }

    #[test]
    fn test_envs_arg_unicode() {
        let result = CommandLineArgs::try_parse_from(["--envs=CAFÉ=café"]).unwrap();
        assert_eq!(
            result.envs[0],
            vec![(OsString::from("CAFÉ"), OsString::from("café"))]
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_env_from_env_var() {
        std::env::set_var("TEST_PARSE_ENV_VAR", "resolved_value");
        let result = parse_envs("TEST_PARSE_ENV_VAR").unwrap();
        assert_eq!(
            result,
            vec![(
                OsString::from("TEST_PARSE_ENV_VAR"),
                OsString::from("resolved_value")
            )]
        );
        std::env::remove_var("TEST_PARSE_ENV_VAR");
    }

    #[test]
    fn test_parse_envs_empty() {
        let result = parse_envs("").unwrap();
        assert_eq!(result, vec![]);
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_envs_missing_env_var() {
        let result = parse_envs("NONEXISTENT_VAR_XYZ123").unwrap();
        assert_eq!(result, vec![]);
    }

    #[rstest]
    #[case::empty_key("=value", "Empty key for value: 'value'")]
    #[case::just_equals("=", "Empty key for value: ''")]
    #[case::shlex_error_wrong_quoting(
        "key='value",
        "Failed splitting 'key='value' for POSIX shell environment"
    )]
    fn test_parse_envs_when_error(#[case] input: &str, #[case] expected: &str) {
        let err = parse_envs(input).unwrap_err();
        assert_eq!(err, expected);
    }

    #[rstest]
    #[case::whitespace_only("      ", vec![])]
    #[case::leading_trailing("  A=1  ", vec![(OsString::from("A"), OsString::from("1"))])]
    #[case::multiple_spaces("A=1  B=2", vec![
        (OsString::from("A"), OsString::from("1")),
        (OsString::from("B"), OsString::from("2"))
    ])]
    fn test_parse_envs_whitespace(
        #[case] input: &str,
        #[case] expected: Vec<(OsString, OsString)>,
    ) {
        let result = parse_envs(input).unwrap();
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::simple("KEY=value", "KEY", "value")]
    #[case::value_with_equals("URL=http://example.com", "URL", "http://example.com")]
    #[case::multiple_equals("A=B=C=D", "A", "B=C=D")]
    #[case::empty_value("KEY=", "KEY", "")]
    #[case::with_colons("PATH=/usr/bin:/bin", "PATH", "/usr/bin:/bin")]
    fn test_parse_envs_with_equals(
        #[case] input: &str,
        #[case] expected_key: &str,
        #[case] expected_value: &str,
    ) {
        let result = parse_envs(input).unwrap();
        assert_eq!(
            result,
            vec![(OsString::from(expected_key), OsString::from(expected_value))]
        );
    }
}
