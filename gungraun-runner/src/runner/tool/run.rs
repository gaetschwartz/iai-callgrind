//! The module responsible for the actual run of the benchmark

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output};

use anyhow::Result;
use log::{debug, error};

use super::config::ToolConfig;
use super::path::ToolOutputPath;
use crate::api::{self, ExitWith, Stream, ValgrindTool};
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::bin_bench::Delay;
use crate::runner::common::{Assistant, ModulePath, Streams};
use crate::runner::meta::Metadata;
use crate::util::resolve_binary_path;

/// The run options for the [`ToolCommand`]
#[derive(Debug, Default, Clone)]
pub struct RunOptions {
    /// Set the current directory of the [`ToolCommand`]
    pub current_dir: Option<PathBuf>,
    /// The optional [`Delay`] to apply to the command
    pub delay: Option<Delay>,
    /// If true, clear the environment variables
    pub env_clear: bool,
    /// The environment variables to pass into the [`ToolCommand`]
    pub envs: Vec<(OsString, OsString)>,
    /// Configuration of the expected exit code/signal
    pub exit_with: Option<ExitWith>,
    /// If present, execute the [`ToolCommand`] in a [`api::Sandbox`]
    pub sandbox: Option<api::Sandbox>,
    /// The `setup` assistant to run if present
    pub setup: Option<Assistant>,
    /// The `stderr`
    pub stderr: Option<api::Stdio>,
    /// The `stdin`
    pub stdin: Option<api::Stdin>,
    /// The `stdout`
    pub stdout: Option<api::Stdio>,
    /// The `teardown` assistant to run if present
    pub teardown: Option<Assistant>,
}

/// The final command to execute
#[derive(Debug)]
pub struct ToolCommand {
    /// TODO: DOCS
    pub command: Command,
    /// TODO: DOCS
    pub nocapture: NoCapture,
    /// TODO: DOCS
    pub tool: ValgrindTool,
}

/// TODO: DOCS
#[derive(Debug)]
pub struct ToolCommandChild {
    /// TODO: DOCS
    pub child: Option<Child>,
    /// TODO: DOCS
    pub executable: PathBuf,
    /// TODO: DOCS
    pub exit_with: Option<ExitWith>,
    /// TODO: DOCS
    pub log_path: ToolOutputPath,
    /// TODO: DOCS
    pub tool: ValgrindTool,
}

impl ToolCommand {
    /// Create new `ToolCommand`
    pub fn new(tool: ValgrindTool, meta: &Metadata, is_default: bool) -> Self {
        let nocapture = if is_default {
            meta.args.nocapture
        } else {
            NoCapture::False
        };

        Self {
            command: meta.into(),
            nocapture,
            tool,
        }
    }

    /// Clear the environment variables
    ///
    /// The `LD_PRELOAD` and `LD_LIBRARY_PATH` variables are skipped. If they are set there's
    /// usually a good reason for it.
    ///
    /// If the tool is `Memcheck`: In order to be able run `Memcheck` without errors, the `PATH`,
    /// `HOME` and `DEBUGINFOD_URLS` variables are skipped.
    pub fn env_clear(&mut self) -> &mut Self {
        debug!("{}: Clearing environment variables", self.tool.id());
        for (key, _) in std::env::vars() {
            match (key.as_str(), self.tool) {
                (key @ ("DEBUGINFOD_URLS" | "PATH" | "HOME"), ValgrindTool::Memcheck)
                | (key @ ("LD_PRELOAD" | "LD_LIBRARY_PATH" | "VALGRIND_LIB"), _) => {
                    debug!(
                        "{}: Clearing environment variables: Skipping {key}",
                        self.tool.id()
                    );
                }
                _ => {
                    self.command.env_remove(key);
                }
            }
        }
        self
    }

    /// Run the `ToolCommand`
    pub fn run(
        mut self,
        config: ToolConfig,
        executable: &Path,
        executable_args: &[OsString],
        run_options: RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        child: Option<&mut Child>,
        streams: Option<&Streams>,
        sandbox_dir: Option<&Path>,
    ) -> Result<ToolCommandChild> {
        debug!(
            "{}: Running with executable '{}'",
            self.tool.id(),
            executable.display()
        );

        let RunOptions {
            env_clear,
            current_dir: run_dir,
            exit_with,
            envs,
            stdin,
            stdout,
            stderr,
            ..
        } = run_options;

        if env_clear {
            debug!("Clearing environment variables");
            self.env_clear();
        }

        match (sandbox_dir, run_dir) {
            (None, None) => {}
            (None, Some(run_dir)) => {
                self.command.current_dir(run_dir);
            }
            (Some(sandbox_dir), None) => {
                self.command.current_dir(sandbox_dir);
            }
            (Some(sandbox_dir), Some(run_dir)) => {
                // If run_dir is absolute uses run_dir otherwise joins the paths
                let path = sandbox_dir.join(run_dir);
                self.command.current_dir(path);
            }
        }

        let mut tool_args = config.args;
        tool_args.set_output_arg(output_path, Option::<&str>::None);
        tool_args.set_log_arg(output_path, Option::<&str>::None);
        tool_args.set_xtree_arg(output_path);
        tool_args.set_xleak_arg(output_path);

        let executable = resolve_binary_path(executable, sandbox_dir)?;
        let args = tool_args.to_vec();
        debug!(
            "{}: Arguments: {}",
            self.tool.id(),
            args.iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join(" ")
        );

        self.command
            .args(tool_args.to_vec())
            .arg(&executable)
            .args(executable_args)
            .envs(envs);

        self.nocapture.apply(&mut self.command, streams)?;

        if let Some(stdin) = stdin {
            stdin
                .apply(&mut self.command, Stream::Stdin, child, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

        // TODO: apply streams??
        if let Some(stdout) = stdout {
            stdout
                .apply(&mut self.command, Stream::Stdout, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

        if let Some(stderr) = stderr {
            stderr
                .apply(&mut self.command, Stream::Stderr, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

        self.command
            .spawn()
            .map(|c| {
                ToolCommandChild::new(
                    self.tool,
                    c,
                    executable.clone(),
                    exit_with,
                    output_path.to_log_output(),
                )
            })
            .map_err(|error| {
                Error::LaunchError(PathBuf::from("valgrind"), error.to_string()).into()
            })
    }
}

impl ToolCommandChild {
    /// TODO: DOCS
    pub fn new(
        tool: ValgrindTool,
        child: Child,
        executable: PathBuf,
        exit_with: Option<ExitWith>,
        log_path: ToolOutputPath,
    ) -> Self {
        Self {
            child: Some(child),
            executable,
            exit_with,
            log_path,
            tool,
        }
    }
}

/// Check the exit code of the [`ToolCommand`] and verify it matches the expected [`ExitWith`]
pub fn check_exit(
    tool: ValgrindTool,
    executable: &Path,
    output: Option<Output>,
    status: ExitStatus,
    output_path: &ToolOutputPath,
    exit_with: Option<&ExitWith>,
) -> Result<Option<Output>> {
    let Some(status_code) = status.code() else {
        return Err(
            Error::new_process_error(tool.id(), output, status, Some(output_path.clone())).into(),
        );
    };

    match (status_code, exit_with) {
        (0i32, None | Some(ExitWith::Code(0i32) | ExitWith::Success)) => Ok(output),
        (0i32, Some(ExitWith::Code(code))) => {
            error!(
                "{}: Expected '{}' to exit with '{}' but it succeeded",
                tool.id(),
                executable.display(),
                code
            );
            Err(
                Error::new_process_error(tool.id(), output, status, Some(output_path.clone()))
                    .into(),
            )
        }
        (0i32, Some(ExitWith::Failure)) => {
            error!(
                "{}: Expected '{}' to fail but it succeeded",
                tool.id(),
                executable.display(),
            );
            Err(
                Error::new_process_error(tool.id(), output, status, Some(output_path.clone()))
                    .into(),
            )
        }
        (_, Some(ExitWith::Failure)) => Ok(output),
        (code, Some(ExitWith::Success)) => {
            error!(
                "{}: Expected '{}' to succeed but it terminated with '{}'",
                tool.id(),
                executable.display(),
                code
            );
            Err(
                Error::new_process_error(tool.id(), output, status, Some(output_path.clone()))
                    .into(),
            )
        }
        (actual_code, Some(ExitWith::Code(expected_code))) if actual_code == *expected_code => {
            Ok(output)
        }
        (actual_code, Some(ExitWith::Code(expected_code))) => {
            error!(
                "{}: Expected '{}' to exit with '{}' but it terminated with '{}'",
                tool.id(),
                executable.display(),
                expected_code,
                actual_code
            );
            Err(
                Error::new_process_error(tool.id(), output, status, Some(output_path.clone()))
                    .into(),
            )
        }
        _ => Err(
            Error::new_process_error(tool.id(), output, status, Some(output_path.clone())).into(),
        ),
    }
}
