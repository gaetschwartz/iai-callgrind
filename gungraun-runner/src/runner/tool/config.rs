//! The module containing the [`ToolConfig`] and other related elements

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Result, anyhow};

use super::super::common::Assistant;
use super::args::ToolArgs;
use super::parser::parser_factory;
use super::path::ToolOutputPath;
use super::regression::ToolRegressionConfig;
use super::run::{RunOptions, ToolCommand};
use crate::api::{self, EntryPoint, RawToolArgs, Tool, Tools, ValgrindTool};
use crate::runner::callgrind::flamegraph::Config as FlamegraphConfig;
use crate::runner::common::{Analyzer, CapturedOutput, Config, ModulePath, Sandbox};
use crate::runner::format::OutputFormat;
use crate::runner::meta::Metadata;
use crate::runner::summary::BenchmarkSummary;
use crate::runner::tasks::ProcessHandler;
use crate::runner::{DEFAULT_TOGGLE, cachegrind, callgrind};

/// The tool specific flamegraph configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ToolFlamegraphConfig {
    /// The callgrind configuration
    Callgrind(FlamegraphConfig),
    /// If there is no configuration
    None,
}

/// The [`ToolConfig`] containing the basic configuration values to run the benchmark for this tool
#[derive(Debug, Clone)]
pub struct ToolConfig {
    /// The arguments to pass to the Valgrind executable
    pub args: ToolArgs,
    /// The [`EntryPoint`] of this tool
    pub entry_point: EntryPoint,
    /// The tool specific flamegraph configuration
    pub flamegraph_config: ToolFlamegraphConfig,
    /// The wildcard patterns used to matched a function in the call stack of a program point
    pub frames: Vec<String>,
    /// If true, this tool is the default tool for the benchmark run
    pub is_default: bool,
    /// If true, this tool is enabled for this benchmark
    pub is_enabled: bool,
    /// The tool specific regression check configuration
    pub regression_config: ToolRegressionConfig,
    /// The [`ValgrindTool`]
    pub tool: ValgrindTool,
}

#[derive(Debug)]
struct ToolConfigBuilder {
    entry_point: Option<EntryPoint>,
    flamegraph_config: ToolFlamegraphConfig,
    frames: Vec<String>,
    is_default: bool,
    is_enabled: bool,
    kind: ValgrindTool,
    raw_tool_args: RawToolArgs,
    regression_config: ToolRegressionConfig,
    tool: Option<Tool>,
}

/// Multiple [`ToolConfig`]s
#[derive(Debug, Clone)]
pub struct ToolConfigs(pub Vec<ToolConfig>);

impl ToolConfig {
    /// Creates a new `ToolConfig`.
    pub fn new(
        tool: ValgrindTool,
        is_enabled: bool,
        args: ToolArgs,
        regression_config: ToolRegressionConfig,
        flamegraph_config: ToolFlamegraphConfig,
        entry_point: EntryPoint,
        is_default: bool,
        frames: Vec<String>,
    ) -> Self {
        Self {
            args,
            entry_point,
            flamegraph_config,
            frames,
            is_default,
            is_enabled,
            regression_config,
            tool,
        }
    }
}

impl ToolConfigBuilder {
    fn build(self) -> Result<ToolConfig> {
        let args = match self.kind {
            ValgrindTool::Callgrind => {
                callgrind::args::Args::try_from_raw_tool_args(&[&self.raw_tool_args])?.into()
            }
            ValgrindTool::Cachegrind => {
                cachegrind::args::Args::try_from_raw_tool_args(&[&self.raw_tool_args])?.into()
            }
            _ => ToolArgs::try_from_raw_tool_args(self.kind, &[&self.raw_tool_args])?,
        };

        Ok(ToolConfig::new(
            self.kind,
            self.is_enabled,
            args,
            self.regression_config,
            self.flamegraph_config,
            self.entry_point.unwrap_or(EntryPoint::None),
            self.is_default,
            self.frames.iter().map(Into::into).collect(),
        ))
    }

    /// Build the entry point
    ///
    /// The `default_entry_point` can be different for example for binary benchmarks and library
    /// benchmarks.
    fn entry_point(
        &mut self,
        default_entry_point: &EntryPoint,
        module_path: &ModulePath,
        _id: Option<&String>,
    ) {
        match self.kind {
            ValgrindTool::Callgrind => {
                let entry_point = self
                    .tool
                    .as_ref()
                    .and_then(|t| t.entry_point.clone())
                    .unwrap_or_else(|| default_entry_point.clone());

                match &entry_point {
                    EntryPoint::None => {}
                    EntryPoint::Default => {
                        self.raw_tool_args
                            .extend_ignore_flag(&[format!("toggle-collect={DEFAULT_TOGGLE}")]);
                    }
                    EntryPoint::Custom(custom) => {
                        self.raw_tool_args
                            .extend_ignore_flag(&[format!("toggle-collect={custom}")]);
                    }
                }

                self.entry_point = Some(entry_point);
            }
            ValgrindTool::DHAT => {
                let entry_point = self
                    .tool
                    .as_ref()
                    .and_then(|t| t.entry_point.clone())
                    .unwrap_or_else(|| default_entry_point.clone());

                if entry_point == EntryPoint::Default {
                    // DHAT does not resolve function calls the same way as callgrind does.
                    // Sometimes the benchmark function matched by the `DEFAULT_TOGGLE` gets inlined
                    // (although annotated with `#[inline(never)]`). So, in addition to the default
                    // toggle we need a fall back to the next best thing which is the function that
                    // calls the benchmark function. It is important to note that this function is
                    // constructed in a way so that it does not contain code that initializes
                    // memory. This "id"-function won't be matched literally but with a wildcard to
                    // address the problem of functions with the same body being condensed into a
                    // single function by the compiler. This also addresses rare cases in which the
                    // id function is taken from another module.
                    if let Some(file) = module_path.components().first() {
                        // This frame glob matches the standalone wrapper mod id function
                        // (`__gungraun_wrapper_id_mod`) and the constructed ones (for example
                        // `__gungraun_wrapper_id_mod_my_benchmark_id`) unambiguously.
                        self.frames
                            .push(format!("{file}::*::__gungraun_wrapper_id_mod*::*"));
                    }
                }

                self.entry_point = Some(entry_point);
            }
            ValgrindTool::Cachegrind
            | ValgrindTool::Memcheck
            | ValgrindTool::Helgrind
            | ValgrindTool::DRD
            | ValgrindTool::Massif
            | ValgrindTool::BBV => {}
        }
    }

    fn flamegraph_config(&mut self) {
        if let Some(tool) = &self.tool {
            if let Some(flamegraph_config) = &tool.flamegraph_config {
                self.flamegraph_config = flamegraph_config.clone().into();
            }
        }
    }

    fn meta_args(&mut self, meta: &Metadata) {
        if let Some(args) = &meta.args.valgrind_args {
            self.raw_tool_args.update(args);
        }

        let raw_tool_args = match self.kind {
            ValgrindTool::Callgrind => &meta.args.callgrind_args,
            ValgrindTool::Cachegrind => &meta.args.cachegrind_args,
            ValgrindTool::DHAT => &meta.args.dhat_args,
            ValgrindTool::Memcheck => &meta.args.memcheck_args,
            ValgrindTool::Helgrind => &meta.args.helgrind_args,
            ValgrindTool::DRD => &meta.args.drd_args,
            ValgrindTool::Massif => &meta.args.massif_args,
            ValgrindTool::BBV => &meta.args.bbv_args,
        };

        if let Some(args) = raw_tool_args {
            self.raw_tool_args.update(args);
        }
    }

    fn new(
        valgrind_tool: ValgrindTool,
        tool: Option<Tool>,
        is_default: bool,
        default_args: &HashMap<ValgrindTool, RawToolArgs>,
        module_path: &ModulePath,
        id: Option<&String>,
        meta: &Metadata,
        valgrind_args: &RawToolArgs,
        default_entry_point: &EntryPoint,
    ) -> Result<Self> {
        let (is_enabled, frames) = if let Some(tool) = tool.as_ref() {
            let is_enabled = tool.enable.unwrap_or(true);
            let frames = tool.frames.as_ref().map_or_else(Vec::default, Clone::clone);

            (is_enabled, frames)
        } else {
            (true, Vec::default())
        };

        let mut builder = Self {
            is_enabled,
            frames,
            tool,
            entry_point: Option::default(),
            flamegraph_config: ToolFlamegraphConfig::None,
            is_default,
            raw_tool_args: default_args
                .get(&valgrind_tool)
                .cloned()
                .unwrap_or_default(),
            regression_config: ToolRegressionConfig::None,
            kind: valgrind_tool,
        };

        // Since the construction sequence is currently always the same, the construction of the
        // `ToolConfig` can happen here in one go instead of having a separate director for it.
        builder.valgrind_args(valgrind_args);
        builder.entry_point(default_entry_point, module_path, id);
        builder.tool_args();
        builder.meta_args(meta);
        builder.flamegraph_config();
        builder.regression_config(meta)?;

        Ok(builder)
    }

    fn regression_config(&mut self, meta: &Metadata) -> Result<()> {
        let meta_limits = match self.kind {
            ValgrindTool::Callgrind => meta.args.callgrind_limits.clone(),
            ValgrindTool::Cachegrind => meta.args.cachegrind_limits.clone(),
            ValgrindTool::DHAT => meta.args.dhat_limits.clone(),
            _ => None,
        };

        let mut regression_config = if let Some(tool) = &self.tool {
            meta_limits
                .map(Ok)
                .or_else(|| tool.regression_config.clone().map(TryInto::try_into))
                .transpose()
                .map_err(|error| anyhow!("Invalid limits for {}: {error}", self.kind))?
                .unwrap_or(ToolRegressionConfig::None)
        } else {
            meta_limits.unwrap_or(ToolRegressionConfig::None)
        };

        if let Some(fail_fast) = meta.args.regression_fail_fast {
            match &mut regression_config {
                ToolRegressionConfig::Callgrind(callgrind_regression_config) => {
                    callgrind_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::Cachegrind(cachegrind_regression_config) => {
                    cachegrind_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::Dhat(dhat_regression_config) => {
                    dhat_regression_config.fail_fast = fail_fast;
                }
                ToolRegressionConfig::None => {}
            }
        }

        self.regression_config = regression_config;

        Ok(())
    }

    fn tool_args(&mut self) {
        if let Some(tool) = self.tool.as_ref() {
            self.raw_tool_args.update(&tool.raw_tool_args);
        }
    }

    fn valgrind_args(&mut self, valgrind_args: &RawToolArgs) {
        self.raw_tool_args.update(valgrind_args);
    }
}

impl ToolConfigs {
    /// Creates new `ToolConfigs`.
    ///
    /// `default_entry_point` is callgrind specific and specified here because it is different for
    /// library and binary benchmarks.
    ///
    /// `default_args` should only contain command-line arguments which are different for library
    /// and binary benchmarks on a per tool basis. Usually, default arguments are part of the tool
    /// specific `Args` struct for example for callgrind [`callgrind::args::Args`] or cachegrind
    /// [`cachegrind::args::Args`].
    ///
    /// `valgrind_args` are from the in-benchmark configuration: `LibraryBenchmarkConfig` or
    /// `BinaryBenchmarkConfig`
    ///
    /// # Errors
    ///
    /// This function will return an error if the configs cannot be created
    pub fn new(
        output_format: &mut OutputFormat,
        mut tools: Tools,
        module_path: &ModulePath,
        id: Option<&String>,
        meta: &Metadata,
        default_tool: ValgrindTool,
        default_entry_point: &EntryPoint,
        valgrind_args: &RawToolArgs,
        default_args: &HashMap<ValgrindTool, RawToolArgs>,
    ) -> Result<Self> {
        let extracted_tool = tools.consume(default_tool);

        output_format.update(extracted_tool.as_ref());
        let default_tool_config = ToolConfigBuilder::new(
            default_tool,
            extracted_tool,
            true,
            default_args,
            module_path,
            id,
            meta,
            valgrind_args,
            default_entry_point,
        )?
        .build()?;

        // The tool selection from the command line or env args overwrites the tool selection from
        // the benchmark file. However, any tool configurations from the benchmark files are
        // preserved.
        let meta_tools = if meta.args.tools.is_empty() {
            tools.0
        } else {
            let mut meta_tools = Vec::with_capacity(meta.args.tools.len());
            for kind in &meta.args.tools {
                if let Some(tool) = tools.consume(*kind) {
                    meta_tools.push(tool);
                } else {
                    meta_tools.push(Tool::new(*kind));
                }
            }
            meta_tools
        };

        let mut tool_configs = Self(vec![default_tool_config]);
        tool_configs.extend(meta_tools.into_iter().map(|tool| {
            output_format.update(Some(&tool));

            ToolConfigBuilder::new(
                tool.kind,
                Some(tool),
                false,
                default_args,
                module_path,
                id,
                meta,
                valgrind_args,
                default_entry_point,
            )?
            .build()
        }))?;

        output_format.update_from_meta(meta);
        Ok(tool_configs)
    }

    /// Returns `true` if there are any [`Tool`]s enabled.
    pub fn has_tools_enabled(&self) -> bool {
        self.0.iter().any(|t| t.is_enabled)
    }

    /// Returns `true` if there are multiple tools configured and are enabled.
    pub fn has_multiple(&self) -> bool {
        self.0.len() > 1 && self.0.iter().filter(|f| f.is_enabled).count() > 1
    }

    /// Returns the parser and configurations for each tool to be able to analyze the outputs.
    pub fn analyzers(&self, root_dir: &Path, output_path: &ToolOutputPath) -> Vec<Analyzer> {
        self.0
            .iter()
            .filter(|t| t.is_enabled)
            .map(|t| {
                let tool_path = output_path.to_tool_output(t.tool);
                (
                    parser_factory(t, root_dir.to_path_buf(), &tool_path),
                    tool_path,
                    t.regression_config.clone(),
                    t.flamegraph_config.clone(),
                    t.entry_point.clone(),
                )
            })
            .collect()
    }

    /// Return all [`ToolOutputPath`]s of all enabled tools
    pub fn output_paths(&self, output_path: &ToolOutputPath) -> Vec<ToolOutputPath> {
        self.0
            .iter()
            .filter(|t| t.is_enabled)
            .map(|t| output_path.to_tool_output(t.tool))
            .collect()
    }

    /// Extends this collection of tools with the contents of an iterator.
    pub fn extend<I>(&mut self, iter: I) -> Result<()>
    where
        I: Iterator<Item = Result<ToolConfig>>,
    {
        for a in iter {
            self.0.push(a?);
        }

        Ok(())
    }

    /// Run a benchmark with this configuration if not --load-baseline was given
    pub fn run(
        &self,
        benchmark_summary: BenchmarkSummary,
        config: &Config,
        executable: &Path,
        executable_args: &[OsString],
        run_options: &RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        captured_output: Option<&CapturedOutput>,
        force_shutdown: &Arc<AtomicBool>,
    ) -> Result<BenchmarkSummary> {
        for tool_config in self.0.iter().filter(|t| t.is_enabled) {
            let tool = tool_config.tool;

            let output_path = output_path.to_tool_output(tool);

            // We're implicitly applying the default here: In the absence of a user provided sandbox
            // we don't run the benchmarks in a sandbox.
            let sandbox = run_options
                .sandbox
                .as_ref()
                .map(|sandbox| Sandbox::setup(sandbox, &config.meta))
                .transpose()?;

            let mut process_handler = ProcessHandler::new(
                force_shutdown.clone(),
                module_path.clone(),
                run_options
                    .setup
                    .as_ref()
                    .is_some_and(Assistant::is_parallel),
                Duration::from_millis(50),
                sandbox.as_ref().and_then(Sandbox::path),
            );

            let command = ToolCommand::new(tool_config, &config.meta, &output_path, run_options)?;
            let nocapture = command.nocapture;
            let captured_output = if tool_config.is_default {
                captured_output
            } else {
                None
            };
            run_options.setup.as_ref().map_or(Ok(()), |setup| {
                process_handler.start_assistant(
                    true,
                    setup,
                    config,
                    module_path,
                    captured_output,
                    nocapture,
                )
            })?;

            if let Some(delay) = run_options.delay.as_ref() {
                if let Err(delay_error) = delay.apply(sandbox.as_ref().and_then(Sandbox::path)) {
                    if let Some(Err(_)) = process_handler.wait_for_setup() {
                        return Err(delay_error);
                    }
                }
            }

            process_handler
                .start_bench(
                    command,
                    tool_config,
                    executable,
                    executable_args,
                    run_options,
                    &output_path,
                    module_path,
                    captured_output,
                    config.meta.args.valgrind_runner_dest.as_deref(),
                )
                .and_then(|()| process_handler.wait_or_shutdown())?;

            if let Some(teardown) = run_options.teardown.as_ref() {
                process_handler
                    .start_assistant(
                        true,
                        teardown,
                        config,
                        module_path,
                        captured_output,
                        nocapture,
                    )
                    .and_then(|()| process_handler.wait_for_teardown().transpose())?;
            }

            if let Some(sandbox) = sandbox {
                sandbox.reset()?;
            }
        }

        Ok(benchmark_summary)
    }
}

impl From<Option<FlamegraphConfig>> for ToolFlamegraphConfig {
    fn from(value: Option<FlamegraphConfig>) -> Self {
        match value {
            Some(config) => Self::Callgrind(config),
            None => Self::None,
        }
    }
}

impl From<api::ToolFlamegraphConfig> for ToolFlamegraphConfig {
    fn from(value: api::ToolFlamegraphConfig) -> Self {
        match value {
            api::ToolFlamegraphConfig::Callgrind(flamegraph_config) => {
                Self::Callgrind(flamegraph_config.into())
            }
            api::ToolFlamegraphConfig::None => Self::None,
        }
    }
}
