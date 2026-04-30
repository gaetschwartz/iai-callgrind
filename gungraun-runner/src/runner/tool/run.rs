//! The module responsible for the actual run of the benchmark

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

use anyhow::Result;
use itertools::Itertools;
use log::{debug, error};
use os_str_bytes::OsStrBytesExt;

use super::config::ToolConfig;
use super::path::ToolOutputPath;
use crate::api::{self, ExitWith, Stream, ValgrindTool};
use crate::error::Error;
use crate::runner::args::NoCapture;
use crate::runner::bin_bench::Delay;
use crate::runner::common::{Assistant, CapturedOutput, ModulePath};
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
    pub envs: HashMap<OsString, OsString>,
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

/// A configured valgrind command ready to be executed
///
/// This struct encapsulates a valgrind tool invocation with its command, output capture
/// configuration, and the specific tool being used.
#[derive(Debug)]
pub struct ToolCommand {
    /// The `std::process` command to be spawned
    pub command: Command,
    /// Configuration for whether to capture or pass through the subprocess output
    pub nocapture: NoCapture,
    /// Optional path rebasing configuration for containerized runners
    ///
    /// When using `--valgrind-runner-root`, this contains the tuple `(original_workspace_root,
    /// replacement_path)` for rebasing paths to match the runner's perspective (e.g., inside a
    /// container).
    pub roots: Option<(PathBuf, PathBuf)>,
    /// The [`ValgrindTool`] to run
    pub tool: ValgrindTool,
}

/// A running valgrind tool process and its metadata
///
/// This struct represents an actively spawned valgrind tool process and tracks information needed
/// to monitor its execution and validate its exit status.
#[derive(Debug)]
pub struct ToolCommandChild {
    /// The spawned child process, or `None` if the process has already been consumed
    pub child: Option<Child>,
    /// The path to the executable being profiled by valgrind
    pub executable: PathBuf,
    /// The expected exit behavior (exit code or signal), or `None` if any exit is acceptable
    pub exit_with: Option<ExitWith>,
    /// The path where Valgrind will write its output log files
    pub log_path: ToolOutputPath,
    /// The Valgrind tool running this process (e.g., Memcheck, Callgrind, Massif)
    pub tool: ValgrindTool,
}

impl ToolCommand {
    /// Creates new `ToolCommand`.
    pub fn new(
        tool_config: &ToolConfig,
        meta: &Metadata,
        output_path: &ToolOutputPath,
        run_options: &RunOptions,
    ) -> Result<Self> {
        let nocapture = if tool_config.is_default {
            meta.args.nocapture
        } else {
            NoCapture::False
        };

        let command = meta.to_tool_command(tool_config, output_path, run_options)?;
        Ok(Self {
            command,
            nocapture,
            tool: tool_config.tool,
            roots: meta
                .args
                .valgrind_runner_root
                .clone()
                .map(|r| (meta.project_root.clone(), r)),
        })
    }

    /// Resolve an executable path, applying path rebasing if configured
    ///
    /// When `--valgrind-runner-root` is specified, this method attempts to rebase the executable
    /// path from the original workspace root to the runner's perspective. If rebasing is not
    /// possible or not configured, falls back to resolving the binary path normally.
    pub fn resolve_executable(&self, executable: &Path, current_dir: Option<&Path>) -> PathBuf {
        if let Some(rebased) = self.try_rebase_arg(executable.as_os_str()) {
            PathBuf::from(rebased)
        } else {
            resolve_binary_path(executable, current_dir)
                .unwrap_or_else(|_| executable.to_path_buf())
        }
    }

    /// Add an argument to the command
    ///
    /// This is a convenience wrapper around `self.command.arg()`.
    pub fn arg<T>(&mut self, arg: T) -> &mut Self
    where
        T: AsRef<OsStr>,
    {
        self.command.arg(arg.as_ref());
        self
    }

    /// Add an argument to the command, applying path rebasing if configured
    ///
    /// When `--valgrind-runner-root` is specified and the argument appears to be a path that needs
    /// rebasing, this method will rebase it. Otherwise, it behaves like [`Self::arg`].
    pub fn arg_rebase<T>(&mut self, arg: T) -> &mut Self
    where
        T: AsRef<OsStr>,
    {
        let arg = arg.as_ref();

        if let Some(rebased) = self.try_rebase_arg(arg) {
            self.command.arg(rebased);
        } else {
            self.command.arg(arg);
        }

        self
    }

    /// Add multiple arguments to the command
    ///
    /// This is a convenience wrapper that calls [`Self::arg`] for each argument.
    pub fn args<I, T>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    /// Add multiple arguments to the command, applying path rebasing if configured
    ///
    /// This is a convenience wrapper that calls [`Self::arg_rebase`] for each argument.
    pub fn args_rebase<I, T>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<OsStr>,
    {
        for arg in args {
            self.arg_rebase(arg);
        }
        self
    }

    /// Run the `ToolCommand`
    pub fn run(
        mut self,
        config: &ToolConfig,
        executable: &Path,
        executable_args: &[OsString],
        run_options: &RunOptions,
        output_path: &ToolOutputPath,
        module_path: &ModulePath,
        child: Option<&mut Child>,
        captured_output: Option<&CapturedOutput>,
        sandbox_dir: Option<&Path>,
        valgrind_runner_dest: Option<&Path>,
    ) -> Result<ToolCommandChild> {
        let RunOptions {
            current_dir,
            exit_with,
            stdin,
            stdout,
            stderr,
            ..
        } = run_options.clone();

        match (sandbox_dir, current_dir.as_ref()) {
            (None, None) => {}
            (None, Some(current_dir)) => {
                self.command.current_dir(current_dir);
            }
            (Some(sandbox_dir), None) => {
                self.command.current_dir(sandbox_dir);
            }
            (Some(sandbox_dir), Some(current_dir)) => {
                // If run_dir is absolute uses run_dir otherwise joins the paths
                let path = sandbox_dir.join(current_dir);
                self.command.current_dir(path);
            }
        }

        let mut tool_args = config.args.clone();
        tool_args.set_output_arg(output_path, valgrind_runner_dest);
        tool_args.set_log_arg(output_path, valgrind_runner_dest);
        tool_args.set_xtree_arg(output_path, valgrind_runner_dest);
        tool_args.set_xleak_arg(output_path, valgrind_runner_dest);

        let executable = self.resolve_executable(executable, sandbox_dir);
        debug!("{}: Executable: {}", self.tool.id(), executable.display());
        debug!(
            "{}: Executable arguments: {}",
            self.tool.id(),
            executable_args
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .join(" ")
        );

        let args = tool_args.to_vec();
        debug!(
            "{}: Valgrind arguments: {}",
            self.tool.id(),
            args.iter()
                .map(|s| s.to_string_lossy().to_string())
                .join(" ")
        );

        self.args_rebase(args)
            .arg(&executable) // already resolved, no need to rebase
            .args_rebase(executable_args);

        self.nocapture.apply(&mut self.command, captured_output)?;

        if let Some(stdin) = stdin {
            stdin
                .apply(&mut self.command, Stream::Stdin, child, sandbox_dir)
                .map_err(|error| Error::BenchmarkError(self.tool, module_path.clone(), error))?;
        }

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
                    executable,
                    exit_with,
                    output_path.to_log_output(),
                )
            })
            .map_err(|error| {
                Error::LaunchError(PathBuf::from("valgrind"), error.to_string()).into()
            })
    }

    /// Attempts to rebase a path argument if it starts with the workspace root.
    ///
    /// Returns `Some(rebased_arg)` if rebasing was successful, `None` if the argument
    /// should be passed through unchanged.
    fn try_rebase_arg(&self, arg: &OsStr) -> Option<OsString> {
        let (workspace_root, new_root) = self.roots.as_ref()?;

        if arg.starts_with("-") {
            if let Some((key, value)) = arg.split_once("=") {
                Self::try_rebase_path_arg(key, value, workspace_root, new_root, "=")
            } else if let Some((key, value)) = arg.split_once(" ") {
                Self::try_rebase_path_arg(key, value, workspace_root, new_root, " ")
            } else {
                None
            }
        } else {
            Path::new(arg)
                .strip_prefix(workspace_root)
                .ok()
                .map(|suffix| new_root.join(suffix).into_os_string())
        }
    }

    /// Attempts to rebase a key-value argument where the value is a path.
    ///
    /// Returns `Some(rebased_arg)` if the value path was successfully rebased,
    /// `None` if the value is not under the workspace root.
    fn try_rebase_path_arg(
        key: &OsStr,
        value: &OsStr,
        workspace_root: &Path,
        new_root: &Path,
        separator: &str,
    ) -> Option<OsString> {
        let suffix = Path::new(value).strip_prefix(workspace_root).ok()?;

        let new_path = new_root.join(suffix);
        let mut new_arg = key.to_os_string();
        new_arg.push(separator);
        new_arg.push(new_path.into_os_string());

        Some(new_arg)
    }
}

impl ToolCommandChild {
    /// Creates a new `ToolCommandChild` instance to manage a spawned tool process.
    ///
    /// This constructor wraps a spawned child process along with metadata needed to track and
    /// manage its execution. The `tool` parameter specifies which [`ValgrindTool`] is being run,
    /// `child` is the actual spawned process, `executable` is the path to the binary being
    /// instrumented, `exit_with` defines the expected exit behavior, and `log_path` specifies
    /// where the tool's output is written.
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
    output: Output,
    output_path: &ToolOutputPath,
    exit_with: Option<&ExitWith>,
) -> Result<Output> {
    let Some(status_code) = output.status.code() else {
        // death by signal
        return Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into());
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
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        (0i32, Some(ExitWith::Failure)) => {
            error!(
                "{}: Expected '{}' to fail but it succeeded",
                tool.id(),
                executable.display(),
            );
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        (_, Some(ExitWith::Failure)) => Ok(output),
        (code, Some(ExitWith::Success)) => {
            error!(
                "{}: Expected '{}' to succeed but it terminated with '{}'",
                tool.id(),
                executable.display(),
                code
            );
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
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
            Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into())
        }
        _ => Err(Error::new_process_error(tool.id(), output, Some(output_path.clone())).into()),
    }
}
