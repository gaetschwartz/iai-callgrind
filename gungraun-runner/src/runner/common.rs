//! This module contains elements which are common to library and binary benchmarks

mod defaults {
    pub const SANDBOX_ENABLED: bool = false;
    pub const SANDBOX_FIXTURES_FOLLOW_SYMLINKS: bool = false;
    pub const COMPARE_BY_ID: bool = false;
}

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Display;
use std::fs::File;
use std::io::{stderr, stdout, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio as StdStdio};
use std::sync::{atomic, Arc};
use std::time::{Duration, Instant};

use anyhow::Result;
use log::{debug, log_enabled, warn, Level};
use tempfile::{tempfile, TempDir};

use super::format::{OutputFormatKind, SummaryFormatter};
use super::meta::Metadata;
use super::summary::BenchmarkSummary;
use crate::api::{self, BinaryBenchmarkGroups, LibraryBenchmarkGroups, Pipe};
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::bin_bench::{self, BinBench};
use crate::runner::format::{
    self, BinaryBenchmarkHeader, Header, LibraryBenchmarkHeader, OutputFormat,
};
use crate::runner::lib_bench::{self, LibBench};
use crate::runner::tasks::ThreadPool;
use crate::util::{copy_directory, make_absolute};

/// The `Baselines` type
pub type Baselines = (Option<String>, Option<String>);

/// TODO: DOCS
#[derive(Debug, Clone)]
pub enum Benches {
    /// TODO: DOCS
    LibBenches(Vec<LibBench>),
    /// TODO: DOCS
    BinBenches(Vec<BinBench>),
}

/// the [`Assistant`] kind
#[derive(Debug, Clone, Copy)]
pub enum AssistantKind {
    /// The `setup` function
    Setup,
    /// The `teardown` function
    Teardown,
}

/// An `Assistant` corresponds to the `setup` or `teardown` functions in the UI
#[derive(Debug, Clone)]
pub struct Assistant {
    envs: Vec<(OsString, OsString)>,
    group_index: Option<usize>,
    indices: Option<(usize, usize, Option<usize>)>,
    kind: AssistantKind,
    pipe: Option<Pipe>,
    run_parallel: bool,
}
/// Contains benchmark summaries of (binary, library) benchmark runs and their execution time
///
/// Used to print a final summary after all benchmarks.
#[derive(Debug, Default)]
pub struct BenchmarkSummaries {
    /// The amount of filtered benchmarks
    pub num_filtered: usize,
    /// The benchmark summaries
    pub summaries: Vec<BenchmarkSummary>,
    /// The execution time of all benchmarks.
    pub total_time: Option<Duration>,
}

/// The `Config` contains all the information extracted from the UI invocation of the runner
#[derive(Debug, Clone)]
pub struct Config {
    /// The path to the compiled binary with the benchmark harness
    pub bench_bin: PathBuf,
    /// The path to the benchmark file which contains the benchmark harness
    pub bench_file: PathBuf,
    /// The [`Metadata`]
    pub meta: Metadata,
    /// The module path of the benchmark file
    pub module_path: ModulePath,
    /// The package directory of the package in which `gungraun` (not the runner) is used
    pub package_dir: PathBuf,
}

// TODO: DOCS
/// A `Group` is the organizational unit and counterpart of the `library_benchmark_group!` macro
#[derive(Debug, Clone)]
pub struct Group {
    /// TODO: DOCS
    pub benches: Benches,
    /// TODO: DOCS
    pub compare_by_id: bool,
    /// TODO: DOCS
    pub index: usize,
    /// TODO: DOCS
    pub module_path: ModulePath,
    /// TODO: DOCS
    pub num_filtered: usize,
    /// TODO: DOCS
    pub setup: Option<Assistant>,
    /// TODO: DOCS
    pub teardown: Option<Assistant>,
}

// TODO: DOCS
/// `Groups` is the top-level organizational unit of the `main!` macro for library benchmarks
#[derive(Debug, Clone)]
pub struct Groups(pub Vec<Group>);

/// TODO: DOCS
#[derive(Debug)]
pub struct JobResult {
    /// TODO: DOCS
    pub benchmark_summary: BenchmarkSummary,
    /// TODO: DOCS
    pub fail_fast: bool,
    /// TODO: DOCS
    pub header: Header,
    /// TODO: DOCS
    pub output_format: OutputFormat,
    /// TODO: DOCS
    pub streams: Streams,
}

/// A helper struct similar to a file path but for module paths with the `::` delimiter
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModulePath(String);

/// The `Sandbox` in which benchmarks should be runs
///
/// As soon as the `Sandbox` is dropped the temporary directory is deleted.
#[derive(Debug)]
pub struct Sandbox {
    temp_dir: Option<TempDir>,
}

/// The main runner to run all library or binary benchmarks
#[derive(Debug)]
pub struct Runner {
    config: Arc<Config>,
    groups: Groups,
    setup: Option<Assistant>,
    teardown: Option<Assistant>,
}

/// TODO: DOCS
#[derive(Debug)]
pub struct Streams {
    /// TODO: DOCS
    pub stderr: File,
    /// TODO: DOCS
    pub stdout: File,
}

impl Assistant {
    /// TODO: DOCS
    pub fn kind(&self) -> AssistantKind {
        self.kind
    }

    /// TODO: DOCS
    pub fn is_parallel(&self) -> bool {
        self.pipe.is_some() || self.run_parallel
    }

    /// The setup or teardown of the `main` macro
    pub fn new_main_assistant(
        kind: AssistantKind,
        envs: Vec<(OsString, OsString)>,
        run_parallel: bool,
    ) -> Self {
        Self {
            kind,
            group_index: None,
            indices: None,
            pipe: None,
            envs,
            run_parallel,
        }
    }

    /// The setup or teardown of a `binary_benchmark_group` or `library_benchmark_group`
    pub fn new_group_assistant(
        kind: AssistantKind,
        group_index: usize,
        envs: Vec<(OsString, OsString)>,
        run_parallel: bool,
    ) -> Self {
        Self {
            kind,
            group_index: Some(group_index),
            indices: None,
            pipe: None,
            envs,
            run_parallel,
        }
    }

    /// The setup or teardown function of a `Bench`
    ///
    /// This is currently only used by binary benchmarks. Library benchmarks use a completely
    /// different logic for setup and teardown functions specified in a `#[bench]`, `#[benches]` and
    /// `#[library_benchmark]` and don't need to be executed via the compiled benchmark.
    pub fn new_bench_assistant(
        kind: AssistantKind,
        group_index: usize,
        indices: (usize, usize, Option<usize>),
        pipe: Option<Pipe>,
        envs: Vec<(OsString, OsString)>,
        run_parallel: bool,
    ) -> Self {
        Self {
            kind,
            group_index: Some(group_index),
            indices: Some(indices),
            pipe,
            envs,
            run_parallel,
        }
    }

    /// The arguments for the benchmark executable
    fn executable_args(&self) -> Vec<OsString> {
        let mut args = vec![OsString::from("--gungraun-run")];

        // The index of the binary or `library_benchmark_group!` in the main! macro
        if let Some(main_index) = &self.group_index {
            args.push(main_index.to_string().into());
        }

        args.push(self.kind.id().into());

        // The `group_index` here is the index in the binary or `library_benchmark_group!`
        if let Some((group_index, bench_index, iter_index)) = &self.indices {
            args.extend([
                group_index.to_string().into(),
                bench_index.to_string().into(),
            ]);
            if let Some(iter_index) = iter_index {
                args.push(iter_index.to_string().into());
            }
        }

        args
    }

    /// Run the `Assistant` by calling the benchmark binary with the needed arguments
    ///
    /// We don't run the assistant if `--load-baseline` was given on the command-line!
    pub fn run(
        &self,
        config: &Config,
        module_path: &ModulePath,
        streams: Option<&Streams>,
        force_parallel: bool,
        current_dir: Option<&Path>,
        nocapture: NoCapture,
    ) -> Result<Option<Child>> {
        if config.meta.args.load_baseline.is_some() {
            return Ok(None);
        }

        let id = self.kind.id();

        let mut command = Command::new(&config.bench_bin);
        command.envs(self.envs.iter().cloned());
        command.args(self.executable_args());

        if let Some(current_dir) = current_dir {
            command.current_dir(current_dir);
        }

        nocapture.apply(&mut command, streams)?;

        match &self.pipe {
            Some(Pipe::Stdout) => {
                command.stdout(StdStdio::piped());
            }
            Some(Pipe::Stderr) => {
                command.stderr(StdStdio::piped());
            }
            _ => {}
        }

        if force_parallel || self.is_parallel() {
            debug!("Spawning assistant '{}' in parallel", self.kind.id());
            let child = command
                .spawn()
                .map_err(|error| Error::LaunchError(config.bench_bin.clone(), error.to_string()))?;
            return Ok(Some(child));
        }

        debug!("Running assistant '{}' serially", self.kind.id());
        command
            .output()
            .map_err(|error| Error::LaunchError(config.bench_bin.clone(), error.to_string()))
            .and_then(|output| {
                if output.status.success() {
                    Ok(output)
                } else {
                    let status = output.status;
                    Err(Error::new_process_error(
                        module_path.join(&id).to_string(),
                        Some(output),
                        status,
                        None,
                    ))
                }
            })?;

        Ok(None)
    }
}

impl AssistantKind {
    /// Return the assistant kind `id` as string
    pub fn id(&self) -> String {
        match self {
            Self::Setup => "setup",
            Self::Teardown => "teardown",
        }
        .to_owned()
    }
}

impl Benches {
    /// TODO: DOCS
    pub fn len(&self) -> usize {
        match self {
            Self::LibBenches(lib_benches) => lib_benches.len(),
            Self::BinBenches(bin_benches) => bin_benches.len(),
        }
    }

    /// TODO: DOCS
    pub fn push_lib_bench(&mut self, lib_bench: LibBench) {
        match self {
            Self::LibBenches(lib_benches) => lib_benches.push(lib_bench),
            Self::BinBenches(_) => {}
        }
    }

    /// TODO: DOCS
    pub fn push_bin_bench(&mut self, bin_bench: BinBench) {
        match self {
            Self::LibBenches(_) => {}
            Self::BinBenches(bin_benches) => bin_benches.push(bin_bench),
        }
    }

    /// TODO: DOCS
    pub fn is_empty(&self) -> bool {
        match self {
            Self::LibBenches(lib_benches) => lib_benches.is_empty(),
            Self::BinBenches(bin_benches) => bin_benches.is_empty(),
        }
    }
}

impl BenchmarkSummaries {
    /// Add a [`BenchmarkSummary`]
    pub fn add_summary(&mut self, summary: BenchmarkSummary) {
        self.summaries.push(summary);
    }

    /// Add another `BenchmarkSummary`
    ///
    /// Ignores the execution time.
    pub fn add_other(&mut self, other: Self) {
        other.summaries.into_iter().for_each(|s| {
            self.add_summary(s);
        });
    }

    /// Return true if any regressions were encountered
    pub fn is_regressed(&self) -> bool {
        self.summaries.iter().any(BenchmarkSummary::is_regressed)
    }

    /// Set the total execution from `start` to `now`
    pub fn elapsed(&mut self, start: Instant) {
        self.total_time = Some(start.elapsed());
    }

    /// Return the number of total benchmarks
    pub fn num_benchmarks(&self) -> usize {
        self.summaries.len()
    }

    /// Print the summary if not prevented by command-line arguments
    ///
    /// If `nosummary` is true or [`OutputFormatKind`] is any kind of `JSON` format the summary is
    /// not printed.
    pub fn print(&self, nosummary: bool, output_format_kind: OutputFormatKind) {
        if !nosummary {
            SummaryFormatter::new(output_format_kind).print(self);
        }
    }
}

impl Group {
    /// TODO: DOCS
    pub fn list(self) -> u64 {
        let mut sum = 0u64;
        match self.benches {
            Benches::LibBenches(lib_benches) => {
                for bench in lib_benches {
                    sum += 1;
                    format::print_list_benchmark(&bench.module_path, bench.id.as_ref());
                }
            }
            Benches::BinBenches(bin_benches) => {
                for bench in bin_benches {
                    sum += 1;
                    format::print_list_benchmark(&bench.module_path, bench.id.as_ref());
                }
            }
        }

        sum
    }

    fn start_bin_benches(
        config: &Arc<Config>,
        module_path: &Arc<ModulePath>,
        thread_pool: &mut ThreadPool<Result<JobResult>>,
        benches: Vec<BinBench>,
    ) {
        for bench in benches {
            let fail_fast = bench.is_fail_fast();
            let benchmark: Arc<dyn bin_bench::Benchmark> = bin_bench::benchmark_factory(config);
            let header = BinaryBenchmarkHeader::new(&config.meta, &bench);

            let config = Arc::clone(config);
            let module_path = Arc::clone(module_path);

            let output_format = bench.output_format.clone();

            thread_pool.execute(move |force_shutdown| {
                let streams = Streams::new()?;

                match benchmark.run(
                    bench,
                    &config,
                    &module_path,
                    Some(streams.try_clone()?),
                    &force_shutdown,
                ) {
                    Ok(benchmark_summary) => Ok(JobResult {
                        benchmark_summary,
                        fail_fast,
                        header: header.into(),
                        output_format,
                        streams,
                    }),
                    Err(error) => Err(anyhow::Error::from(Error::JobError(
                        Box::new(error),
                        header.into(),
                        streams,
                    ))),
                }
            });
        }
    }

    fn start_lib_benches(
        main_index: usize,
        config: &Arc<Config>,
        module_path: &Arc<ModulePath>,
        thread_pool: &mut ThreadPool<Result<JobResult>>,
        benches: Vec<LibBench>,
    ) {
        for bench in benches {
            let fail_fast = bench.is_fail_fast();
            let benchmark: Arc<dyn lib_bench::Benchmark> = lib_bench::benchmark_factory(config);
            let header = LibraryBenchmarkHeader::new(&bench);

            let config = Arc::clone(config);
            let module_path = Arc::clone(module_path);

            let output_format = bench.output_format.clone();

            thread_pool.execute(move |force_shutdown| {
                let streams = Streams::new()?;

                match benchmark.run(
                    bench,
                    &config,
                    &module_path,
                    main_index,
                    Some(streams.try_clone()?),
                    &force_shutdown,
                ) {
                    Ok(benchmark_summary) => Ok(JobResult {
                        benchmark_summary,
                        fail_fast,
                        header: header.into(),
                        output_format,
                        streams,
                    }),
                    Err(error) => Err(anyhow::Error::from(Error::JobError(
                        Box::new(error),
                        header.into(),
                        streams,
                    ))),
                }
            });
        }
    }

    /// TODO: DOCS
    pub fn run(self, config: &Arc<Config>) -> Result<BenchmarkSummaries> {
        let mut benchmark_summaries = BenchmarkSummaries::default();

        let mut thread_pool = ThreadPool::<Result<JobResult>>::new(config.meta.args.parallel)?;
        let compare_by_id = self.compare_by_id;
        let num_benches = self.benches.len();
        let module_path = Arc::new(self.module_path.clone());
        let main_index = self.index;

        match self.benches {
            Benches::LibBenches(lib_benches) => {
                Self::start_lib_benches(
                    main_index,
                    config,
                    &module_path,
                    &mut thread_pool,
                    lib_benches,
                );
            }
            Benches::BinBenches(bin_benches) => {
                Self::start_bin_benches(config, &module_path, &mut thread_pool, bin_benches);
            }
        }

        let mut lib_bench_summaries: HashMap<String, Vec<BenchmarkSummary>> =
            HashMap::with_capacity(num_benches);
        let force_shutdown = thread_pool.get_force_shutdown();
        let mut error = None;
        for result in thread_pool {
            let JobResult {
                benchmark_summary,
                fail_fast,
                header,
                output_format,
                streams,
            } = match result {
                // On error, wait for all threads to finish and/or shutdown their running processes
                _ if error.is_some() => {
                    continue;
                }
                Ok(result) => result,
                Err(e) => {
                    force_shutdown.store(true, atomic::Ordering::Release);
                    error = Some(e);
                    continue;
                }
            };

            benchmark_summary.print_and_save(config, &header, &output_format, streams)?;
            benchmark_summary.check_regression(fail_fast)?;

            benchmark_summaries.add_summary(benchmark_summary.clone());
            if compare_by_id && output_format.is_default() {
                if let Some(id) = &benchmark_summary.id {
                    if let Some(sums) = lib_bench_summaries.get_mut(id) {
                        for sum in sums.iter() {
                            sum.compare_and_print(id, &benchmark_summary, &output_format)?;
                        }
                        sums.push(benchmark_summary);
                    } else {
                        lib_bench_summaries.insert(id.clone(), vec![benchmark_summary]);
                    }
                }
            }
        }

        // At this point the thread pool iterator either processed all jobs or aborted them on
        // error, so the threads are idle. On return, whether successful or not, the thread pool
        // will be dropped and all threads are properly closed.
        if let Some(error) = error {
            return Err(error);
        }

        Ok(benchmark_summaries)
    }
}

impl Groups {
    /// TODO: DOCS
    #[allow(clippy::too_many_lines)]
    pub fn from_binary_benchmark(
        module_path: &ModulePath,
        benchmark_groups: BinaryBenchmarkGroups,
        meta: &Metadata,
    ) -> Result<Self> {
        let global_config = benchmark_groups.config;
        let default_tool = benchmark_groups.default_tool;

        let mut groups = vec![];
        for (main_index, binary_benchmark_group) in benchmark_groups.groups.into_iter().enumerate()
        {
            let group_module_path = module_path.join(&binary_benchmark_group.id);
            let group_config = global_config
                .clone()
                .update_from_all([binary_benchmark_group.config.as_ref()]);

            let setup = binary_benchmark_group
                .has_setup
                .then_some(Assistant::new_group_assistant(
                    AssistantKind::Setup,
                    main_index,
                    group_config.collect_envs(),
                    false,
                ));
            let teardown =
                binary_benchmark_group
                    .has_teardown
                    .then_some(Assistant::new_group_assistant(
                        AssistantKind::Teardown,
                        main_index,
                        group_config.collect_envs(),
                        false,
                    ));

            let mut group = Group {
                index: main_index,
                benches: Benches::BinBenches(vec![]),
                compare_by_id: binary_benchmark_group
                    .compare_by_id
                    .unwrap_or(defaults::COMPARE_BY_ID),
                module_path: group_module_path,
                num_filtered: 0,
                setup,
                teardown,
            };

            for (group_index, binary_benchmark_benches) in binary_benchmark_group
                .binary_benchmarks
                .into_iter()
                .enumerate()
            {
                for (bench_index, binary_benchmark_bench) in
                    binary_benchmark_benches.benches.into_iter().enumerate()
                {
                    let module_path = group
                        .module_path
                        .join(&binary_benchmark_bench.function_name);

                    match &binary_benchmark_bench.command {
                        api::CommandKind::Default(command) => {
                            let config = group_config.clone().update_from_all([
                                binary_benchmark_benches.config.as_ref(),
                                binary_benchmark_bench.config.as_ref(),
                                Some(&command.config),
                            ]);

                            let bin_bench = BinBench::new(
                                binary_benchmark_bench.id,
                                binary_benchmark_bench.args,
                                module_path,
                                binary_benchmark_bench.function_name,
                                binary_benchmark_bench.has_setup,
                                binary_benchmark_bench.has_teardown,
                                meta,
                                main_index,
                                config,
                                group_index,
                                bench_index,
                                None,
                                *command.clone(),
                                default_tool,
                            )?;
                            if let Some(bin_bench) = bin_bench {
                                group.benches.push_bin_bench(bin_bench);
                            } else {
                                group.num_filtered += 1;
                            }
                        }
                        api::CommandKind::Iter(commands) => {
                            match (commands.len(), &binary_benchmark_bench.id) {
                                (0, Some(id)) => {
                                    warn!(
                                        "The iterator of {module_path} with id '{id}' was empty."
                                    );
                                }
                                (0, None) => {
                                    warn!("The iterator of {module_path} was empty.");
                                }
                                _ => {
                                    for (iter_index, command) in commands.iter().enumerate() {
                                        let config = group_config.clone().update_from_all([
                                            binary_benchmark_benches.config.as_ref(),
                                            binary_benchmark_bench.config.as_ref(),
                                            Some(&command.config),
                                        ]);

                                        let bin_bench = BinBench::new(
                                            binary_benchmark_bench.id.clone(),
                                            binary_benchmark_bench.args.clone(),
                                            module_path.clone(),
                                            binary_benchmark_bench.function_name.clone(),
                                            binary_benchmark_bench.has_setup,
                                            binary_benchmark_bench.has_teardown,
                                            meta,
                                            main_index,
                                            config,
                                            group_index,
                                            bench_index,
                                            Some(iter_index),
                                            command.clone(),
                                            default_tool,
                                        )?;
                                        if let Some(bin_bench) = bin_bench {
                                            group.benches.push_bin_bench(bin_bench);
                                        } else {
                                            group.num_filtered += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            groups.push(group);
        }
        Ok(Self(groups))
    }

    /// TODO: DOCS
    #[allow(clippy::too_many_lines)]
    pub fn from_library_benchmark(
        module_path: &ModulePath,
        benchmark_groups: LibraryBenchmarkGroups,
        meta: &Metadata,
    ) -> Result<Self> {
        let global_config = benchmark_groups.config;
        let default_tool = benchmark_groups.default_tool;

        let mut groups = vec![];
        for (main_index, library_benchmark_group) in benchmark_groups.groups.into_iter().enumerate()
        {
            let group_module_path = module_path.join(&library_benchmark_group.id);
            let group_config = global_config
                .clone()
                .update_from_all([library_benchmark_group.config.as_ref()]);

            let setup =
                library_benchmark_group
                    .has_setup
                    .then_some(Assistant::new_group_assistant(
                        AssistantKind::Setup,
                        main_index,
                        group_config.collect_envs(),
                        false,
                    ));
            let teardown =
                library_benchmark_group
                    .has_teardown
                    .then_some(Assistant::new_group_assistant(
                        AssistantKind::Teardown,
                        main_index,
                        group_config.collect_envs(),
                        false,
                    ));

            let mut group = Group {
                index: main_index,
                benches: Benches::LibBenches(vec![]),
                compare_by_id: library_benchmark_group
                    .compare_by_id
                    .unwrap_or(defaults::COMPARE_BY_ID),
                module_path: group_module_path,
                num_filtered: 0,
                setup,
                teardown,
            };

            for (group_index, library_benchmark_benches) in library_benchmark_group
                .library_benchmarks
                .into_iter()
                .enumerate()
            {
                for (bench_index, library_benchmark_bench) in
                    library_benchmark_benches.benches.into_iter().enumerate()
                {
                    let config = group_config.clone().update_from_all([
                        library_benchmark_benches.config.as_ref(),
                        library_benchmark_bench.config.as_ref(),
                    ]);

                    let module_path = group
                        .module_path
                        .join(&library_benchmark_bench.function_name);

                    if let Some(iter_count) = library_benchmark_bench.iter_count {
                        match (iter_count, &library_benchmark_bench.id) {
                            (0, Some(id)) => {
                                warn!("The iterator of {module_path} with id '{id}' was empty.");
                            }
                            (0, None) => {
                                warn!("The iterator of {module_path} was empty.");
                            }
                            _ => {
                                for iter_index in 0..iter_count {
                                    let lib_bench = LibBench::new(
                                        library_benchmark_bench.id.clone(),
                                        library_benchmark_bench.args.clone(),
                                        module_path.clone(),
                                        library_benchmark_bench.function_name.clone(),
                                        meta,
                                        config.clone(),
                                        group_index,
                                        bench_index,
                                        Some(iter_index),
                                        default_tool,
                                    )?;
                                    if let Some(lib_bench) = lib_bench {
                                        group.benches.push_lib_bench(lib_bench);
                                    } else {
                                        group.num_filtered += 1;
                                    }
                                }
                            }
                        }
                    } else {
                        let lib_bench = LibBench::new(
                            library_benchmark_bench.id,
                            library_benchmark_bench.args,
                            module_path,
                            library_benchmark_bench.function_name,
                            meta,
                            config,
                            group_index,
                            bench_index,
                            None,
                            default_tool,
                        )?;
                        if let Some(lib_bench) = lib_bench {
                            group.benches.push_lib_bench(lib_bench);
                        } else {
                            group.num_filtered += 1;
                        }
                    }
                }
            }

            groups.push(group);
        }

        Ok(Self(groups))
    }

    /// TODO: DOCS
    pub fn has_benchmarks(&self) -> bool {
        self.0.iter().any(|g| !g.benches.is_empty())
    }

    /// TODO: DOCS
    pub fn list(self) -> Result<()> {
        let mut sum = 0u64;
        for group in self.0 {
            sum += group.list();
        }

        format::print_benchmark_list_summary(sum);

        Ok(())
    }

    /// TODO: DOCS
    pub fn num_filtered(&self) -> usize {
        self.0.iter().fold(0, |acc, group| acc + group.num_filtered)
    }

    /// TODO: DOCS
    pub fn run(self, config: &Arc<Config>) -> Result<BenchmarkSummaries> {
        let mut benchmark_summaries = BenchmarkSummaries::default();

        for group in self.0 {
            if let Some(setup) = &group.setup {
                setup.run(
                    config,
                    &group.module_path,
                    None,
                    false,
                    None,
                    config.meta.args.nocapture,
                )?;
            }

            let module_path = group.module_path.clone();
            let teardown = group.teardown.clone();

            let summaries = group.run(config)?;

            if let Some(teardown) = teardown {
                teardown.run(
                    config,
                    &module_path,
                    None,
                    false,
                    None,
                    config.meta.args.nocapture,
                )?;
            }

            benchmark_summaries.add_other(summaries);
        }

        Ok(benchmark_summaries)
    }
}

impl ModulePath {
    /// Create a new `ModulePath`
    ///
    /// There is no validity check if the path contains valid characters or not and the path is
    /// created as is.
    pub fn new(path: &str) -> Self {
        Self(path.to_owned())
    }

    /// Join this module path with another string (unchecked)
    #[must_use]
    pub fn join(&self, path: &str) -> Self {
        let new = format!("{}::{path}", self.0);
        Self(new)
    }

    /// Return the module path as string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the first segment of the module path if any
    pub fn first(&self) -> Option<Self> {
        self.0
            .split_once("::")
            .map(|(first, _)| Self::new(first))
            .or_else(|| (!self.0.is_empty()).then_some(self.clone()))
    }

    /// Return the last segment of the module path if any
    pub fn last(&self) -> Option<Self> {
        self.0.rsplit_once("::").map(|(_, last)| Self::new(last))
    }

    /// Return the parent module path if present
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once("::")
            .map(|(prefix, _)| Self::new(prefix))
    }

    /// Return a vector which contains all segments of the module path without the delimiter
    pub fn components(&self) -> Vec<&str> {
        self.0.split("::").collect()
    }
}

impl Display for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Runner {
    /// TODO: DOCS
    pub fn has_benchmarks(&self) -> bool {
        self.groups.has_benchmarks()
    }

    /// TODO: DOCS
    pub fn num_filtered(&self) -> usize {
        self.groups.num_filtered()
    }

    /// TODO: DOCS
    pub fn from_library_benchmark(
        benchmark_groups: LibraryBenchmarkGroups,
        config: Config,
    ) -> Result<Self> {
        let setup = benchmark_groups
            .has_setup
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Setup,
                benchmark_groups.config.collect_envs(),
                false,
            ));
        let teardown = benchmark_groups
            .has_teardown
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Teardown,
                benchmark_groups.config.collect_envs(),
                false,
            ));

        let groups =
            Groups::from_library_benchmark(&config.module_path, benchmark_groups, &config.meta)?;

        Ok(Self {
            config: Arc::new(config),
            groups,
            setup,
            teardown,
        })
    }

    /// TODO: DOCS
    pub fn from_binary_benchmark(
        benchmark_groups: BinaryBenchmarkGroups,
        config: Config,
    ) -> Result<Self> {
        let setup = benchmark_groups
            .has_setup
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Setup,
                benchmark_groups.config.collect_envs(),
                false,
            ));
        let teardown = benchmark_groups
            .has_teardown
            .then_some(Assistant::new_main_assistant(
                AssistantKind::Teardown,
                benchmark_groups.config.collect_envs(),
                false,
            ));

        let groups =
            Groups::from_binary_benchmark(&config.module_path, benchmark_groups, &config.meta)?;

        Ok(Self {
            config: Arc::new(config),
            groups,
            setup,
            teardown,
        })
    }

    /// Run all benchmarks in all groups
    pub fn run(self) -> Result<BenchmarkSummaries> {
        let num_filtered = self.num_filtered();

        if self.has_benchmarks() {
            let start = Instant::now();

            if let Some(setup) = &self.setup {
                setup.run(
                    &self.config,
                    &self.config.module_path,
                    None,
                    false,
                    None,
                    self.config.meta.args.nocapture,
                )?;
            }

            let mut summaries = self.groups.run(&self.config)?;

            if let Some(teardown) = &self.teardown {
                teardown.run(
                    &self.config,
                    &self.config.module_path,
                    None,
                    false,
                    None,
                    self.config.meta.args.nocapture,
                )?;
            }

            summaries.elapsed(start);
            summaries.num_filtered = num_filtered;
            Ok(summaries)
        } else {
            Ok(BenchmarkSummaries {
                num_filtered,
                ..Default::default()
            })
        }
    }
}

impl Sandbox {
    /// Setup the `Sandbox` if enabled
    ///
    /// If enabled, create a temporary directory which has a standardized length. Then copy fixtures
    /// into the temporary directory.
    pub fn setup(inner: &api::Sandbox, meta: &Metadata) -> Result<Self> {
        let enabled = inner.enabled.unwrap_or(defaults::SANDBOX_ENABLED);
        let follow_symlinks = inner
            .follow_symlinks
            .unwrap_or(defaults::SANDBOX_FIXTURES_FOLLOW_SYMLINKS);

        let current_dir = std::env::current_dir().map_err(|error| {
            Error::SandboxError(format!("Failed to detect current directory: {error}"))
        })?;

        let temp_dir = if enabled {
            debug!("Creating sandbox");

            let temp_dir = tempfile::tempdir().map_err(|error| {
                Error::SandboxError(format!("Failed creating temporary directory: {error}"))
            })?;

            for fixture in &inner.fixtures {
                if fixture.is_relative() {
                    let absolute_path = make_absolute(&meta.project_root, fixture);
                    copy_directory(&absolute_path, temp_dir.path(), follow_symlinks)?;
                } else {
                    copy_directory(fixture, temp_dir.path(), follow_symlinks)?;
                }
            }

            Some(temp_dir)
        } else {
            debug!(
                "Sandbox disabled: Running benchmarks in current directory '{}'",
                current_dir.display()
            );
            None
        };

        Ok(Self { temp_dir })
    }

    /// Delete the temporary directory if present
    pub fn reset(self) -> Result<()> {
        if let Some(temp_dir) = self.temp_dir {
            if log_enabled!(Level::Debug) {
                debug!("Removing temporary workspace");
                if let Err(error) = temp_dir.close() {
                    debug!("Error trying to delete temporary workspace: {error}");
                }
            } else {
                _ = temp_dir.close();
            }
        }

        Ok(())
    }

    /// TODO: DOCS
    pub fn path(&self) -> Option<&Path> {
        self.temp_dir.as_ref().map(tempfile::TempDir::path)
    }
}

impl Streams {
    /// TODO: DOCS
    pub fn new() -> Result<Self> {
        Ok(Self {
            stdout: tempfile()?,
            stderr: tempfile()?,
        })
    }

    /// TODO: DOCS
    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self {
            stdout: self.stdout.try_clone()?,
            stderr: self.stderr.try_clone()?,
        })
    }

    /// TODO: DOCS
    pub fn reset(&mut self) -> Result<()> {
        self.stdout.flush()?;
        self.stdout.rewind()?;
        self.stderr.flush()?;
        self.stderr.rewind()?;

        Ok(())
    }

    /// TODO: DOCS
    pub fn into_stdio(&self) -> Result<(StdStdio, StdStdio)> {
        let cloned = self.try_clone()?;

        Ok((cloned.stdout.into(), cloned.stderr.into()))
    }

    /// TODO: DOCS
    pub fn dump(&mut self) -> Result<()> {
        self.reset()?;

        let mut stdout_lock = stdout().lock();
        let mut stderr_lock = stderr().lock();

        std::io::copy(&mut self.stdout, &mut stdout_lock)?;
        std::io::copy(&mut self.stderr, &mut stderr_lock)?;

        Ok(())
    }

    /// TODO: DOCS
    pub fn dump_cloned(&self) -> Result<()> {
        let mut streams = self.try_clone()?;

        streams.reset()?;

        let mut stdout_lock = stdout().lock();
        let mut stderr_lock = stderr().lock();

        std::io::copy(&mut streams.stdout, &mut stdout_lock)?;
        std::io::copy(&mut streams.stderr, &mut stderr_lock)?;
        Ok(())
    }

    /// TODO: DOCS
    pub fn dump_stderr(&mut self) -> Result<()> {
        self.stderr.flush()?;
        self.stderr.rewind()?;

        let mut stderr_lock = stderr().lock();
        std::io::copy(&mut self.stderr, &mut stderr_lock)?;

        Ok(())
    }

    /// TODO: DOCS
    pub fn dump_stdout(&mut self) -> Result<()> {
        self.stdout.flush()?;
        self.stdout.rewind()?;

        let mut stdout_lock = stdout().lock();
        std::io::copy(&mut self.stdout, &mut stdout_lock)?;

        Ok(())
    }
}

impl From<ModulePath> for String {
    fn from(value: ModulePath) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty("", None)]
    #[case::single("first", Some("first"))]
    #[case::two("first::second", Some("first"))]
    #[case::three("first::second::third", Some("first"))]
    fn test_module_path_first(#[case] module_path: &str, #[case] expected: Option<&str>) {
        let expected = expected.map(ModulePath::new);
        let actual = ModulePath::new(module_path).first();

        assert_eq!(actual, expected);
    }
}
