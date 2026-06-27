//! Summary data model types serialized to and from Gungraun summary JSON.
//!
//! These types define schema version 6 and are re-exported by `gungraun-summary::v6` for consumers
//! that want direct access to the parsed summary model.

use std::path::PathBuf;

use either_or_both::EitherOrBoth;
#[cfg(feature = "summary")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind, ValgrindTool};
use crate::metrics::model::{Metric, MetricKind, Metrics, MetricsSummary};

/// The version of the summary json schema
pub const SCHEMA_VERSION: &str = "6";

// FIX: Improve documentation of exported structs, ...
/// The `BaselineKind` describing the baseline
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum BaselineKind {
    /// Compare new against `*.old` output files
    Old,
    /// Compare new against a named baseline
    Name(BaselineName),
}

/// The `BenchmarkKind`, differentiating between library and binary benchmarks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum BenchmarkKind {
    /// A library benchmark
    LibraryBenchmark,
    /// A binary benchmark
    BinaryBenchmark,
}

/// The format (json, ...) in which the summary file should be saved or printed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
#[cfg_attr(feature = "runner", derive(clap::ValueEnum))]
pub enum SummaryFormat {
    /// The format in a space optimal json representation without newlines
    Json,
    /// The format in pretty printed json
    PrettyJson,
}

/// The `ToolMetricSummary` contains the `MetricsSummary` distinguished by tool and metric kinds
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum ToolMetricSummary {
    /// If there are no metrics extracted (currently Massif, BBV)
    #[default]
    None,
    /// The error summary of tools which reports errors (Memcheck, Helgrind, DRD)
    ErrorTool(MetricsSummary<ErrorMetric>),
    /// The dhat summary
    Dhat(MetricsSummary<DhatMetric>),
    /// The Callgrind summary
    Callgrind(MetricsSummary<EventKind>),
    /// The Cachegrind summary
    Cachegrind(MetricsSummary<CachegrindMetric>),
}

/// The metrics distinguished per tool class
///
/// The tool classes are: DHAT, error metrics from Memcheck, DRD, Helgrind and Callgrind
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum ToolMetrics {
    /// If there are no metrics extracted from a tool (currently Massif, BBV)
    #[default]
    None,
    /// The metrics of a dhat benchmark
    Dhat(Metrics<DhatMetric>),
    /// The metrics of a tool run which reports errors (Memcheck, Helgrind, DRD)
    ErrorTool(Metrics<ErrorMetric>),
    /// The metrics of a Callgrind benchmark
    Callgrind(Metrics<EventKind>),
    /// The metrics of a Cachegrind benchmark
    Cachegrind(Metrics<CachegrindMetric>),
}

/// A detected performance regression depending on the limit either `Soft` or `Hard`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum ToolRegression {
    /// A performance regression triggered by a soft limit
    Soft {
        /// The metric kind per tool
        metric: MetricKind,
        /// The value of the new benchmark run
        new: Metric,
        /// The value of the old benchmark run
        old: Metric,
        /// The difference between new and old in percent. Serialized as string to preserve
        /// infinity values and avoid null in json.
        #[serde(with = "crate::serde::float_64")]
        #[cfg_attr(feature = "summary", schemars(with = "String"))]
        diff_pct: f64,
        /// The value of the limit which was exceeded to cause a performance regression. Serialized
        /// as string to preserve infinity values and avoid null in json.
        #[serde(with = "crate::serde::float_64")]
        #[cfg_attr(feature = "summary", schemars(with = "String"))]
        limit: f64,
    },
    /// A performance regression triggered by a hard limit
    Hard {
        /// The metric kind per tool
        metric: MetricKind,
        /// The value of the benchmark run
        new: Metric,
        /// The difference between new and the limit
        diff: Metric,
        /// The limit
        limit: Metric,
    },
}

/// A `Baseline` depending on the [`BaselineKind`] which points to the corresponding path
///
/// This baseline is used for comparisons with the new output of valgrind tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct Baseline {
    /// The kind of the `Baseline`
    pub kind: BaselineKind,
    /// The path to the file which is used to compare against the new output
    pub path: PathBuf,
}

/// The name of the baseline
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct BaselineName(pub String);

/// The `BenchmarkSummary` containing all the information of a single benchmark run
///
/// This includes produced files, recorded callgrind events, performance regressions ...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct BenchmarkSummary {
    /// The baselines if any. An absent first baseline indicates that new output was produced. An
    /// absent second baseline indicates the usage of the usual "*.old" output.
    pub baselines: (Option<String>, Option<String>),
    /// The path to the binary which is executed by valgrind. In case of a library benchmark this
    /// is the compiled benchmark file. In case of a binary benchmark this is the path to the
    /// command.
    pub benchmark_exe: PathBuf,
    /// The path to the benchmark file
    pub benchmark_file: PathBuf,
    /// More details describing this benchmark run
    pub details: Option<String>,
    /// The name of the function under test
    pub function_name: String,
    /// The user provided id of this benchmark
    pub id: Option<String>,
    /// Whether this summary describes a library or binary benchmark
    pub kind: BenchmarkKind,
    /// The rust path in the form `bench_file::group::bench`
    pub module_path: String,
    /// The directory of the package
    pub package_dir: PathBuf,
    /// The summary of other valgrind tool runs
    pub profiles: Profiles,
    /// The project's root directory
    pub project_root: PathBuf,
    /// The destination and kind of the summary file
    pub summary_output: Option<SummaryOutput>,
    /// The version of this format. Only backwards incompatible changes cause an increase of the
    /// version
    pub version: String,
}

/// The differences between two `Metrics` as percentage and factor
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct Diffs {
    /// The percentage of the difference between two `Metrics` serialized as string to preserve
    /// infinity values and avoid `null` in json
    #[serde(with = "crate::serde::float_64")]
    #[cfg_attr(feature = "summary", schemars(with = "String"))]
    pub diff_pct: f64,
    /// The factor of the difference between two `Metrics` serialized as string to preserve
    /// infinity values and void `null` in json
    #[serde(with = "crate::serde::float_64")]
    #[cfg_attr(feature = "summary", schemars(with = "String"))]
    pub factor: f64,
}

/// All callgrind flamegraph summaries and their totals
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct FlamegraphSummaries {
    /// The `FlamegraphSummary`s
    pub summaries: Vec<FlamegraphSummary>,
    /// The totals over the `FlamegraphSummary`s
    pub totals: Vec<FlamegraphSummary>,
}

/// The callgrind `FlamegraphSummary` records all created paths for an [`EventKind`] specific
/// flamegraph
///
/// Either the `regular_path`, `old_path` or the `diff_path` are present. Never can all of them be
/// absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct FlamegraphSummary {
    /// If present, the path to the file of the old regular (non-differential) flamegraph
    pub base_path: Option<PathBuf>,
    /// If present, the path to the file of the differential flamegraph
    pub diff_path: Option<PathBuf>,
    /// The `EventKind` of the flamegraph
    pub event_kind: EventKind,
    /// If present, the path to the file of the regular (non-differential) flamegraph
    pub regular_path: Option<PathBuf>,
}

/// The `ToolSummary` containing all information about a valgrind tool run
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct Profile {
    /// Details and information about the created flamegraphs if any
    pub flamegraphs: Vec<FlamegraphSummary>,
    /// The paths to the `*.log` files. All tools produce at least one log file
    pub log_paths: Vec<PathBuf>,
    /// The paths to the `*.out` files. Not all tools produce an output in addition to the log
    /// files
    pub out_paths: Vec<PathBuf>,
    /// The metrics and details about the tool run
    pub summaries: ProfileData,
    /// The Valgrind tool like `DHAT`, `Memcheck` etc.
    pub tool: ValgrindTool,
}

/// The `ToolRun` contains all information about a single tool run with possibly multiple segments
///
/// The total is always present and summarizes all tool run segments. In the special case of a
/// single tool run segment, the total equals the metrics of this segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct ProfileData {
    /// All [`ProfilePart`]s
    pub parts: Vec<ProfilePart>,
    /// The total over the [`ProfilePart`]s
    pub total: ProfileTotal,
}

/// Some additional and necessary information about the tool run segment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct ProfileInfo {
    /// The executed command extracted from Valgrind output
    pub command: String,
    /// More details for example from the logging output of the tool run
    pub details: Option<String>,
    /// The parent pid of this process
    pub parent_pid: Option<i32>,
    /// The part of this tool run (only callgrind)
    pub part: Option<u64>,
    /// The path to the file from the tool run
    pub path: PathBuf,
    /// The pid of this process
    pub pid: i32,
    /// The thread of this tool run (only callgrind)
    pub thread: Option<usize>,
}

/// A single segment of a tool run and if present the comparison with the "old" segment
///
/// A tool run can produce multiple segments, for example for each process and subprocess with
/// (--trace-children).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct ProfilePart {
    /// Details like command, pid, ppid, thread number etc. (see [`ProfileInfo`])
    pub details: EitherOrBoth<ProfileInfo>,
    /// The [`ToolMetricSummary`]
    pub metrics_summary: ToolMetricSummary,
}

/// The total metrics over all [`ProfilePart`]s and if detected any [`ToolRegression`]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct ProfileTotal {
    /// The detected regressions if any
    pub regressions: Vec<ToolRegression>,
    /// The summary of metrics of the tool
    pub summary: ToolMetricSummary,
}

/// The collection of all generated [`Profile`]s
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
#[derive(Default)]
pub struct Profiles(pub Vec<Profile>);

/// Manage the summary output file with this `SummaryOutput`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct SummaryOutput {
    /// The [`SummaryFormat`]
    pub format: SummaryFormat,
    /// The path to the destination file of this summary
    pub path: PathBuf,
}
