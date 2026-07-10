//! The module containing all elements for [`ValgrindArgs`]

/// Module containing the Gungraun defaults for the command line arguments of all tools
#[expect(missing_docs)]
pub mod defaults {
    use super::{FairSched, Vgdb};

    ////////////////////////////////////////////////////
    // Shared defaults between cachegrind and callgrind
    // Set some reasonable cache sizes. The exact sizes matter less than having fixed sizes, since
    // otherwise callgrind would take them from the CPU and make benchmark runs even more
    // incomparable between machines.
    pub const I1: &str = "32768,8,64";
    pub const D1: &str = "32768,8,64";
    pub const LL: &str = "8388608,16,64";
    pub const CACHE_SIM: bool = true;
    ////////////////////////////////////////////////////

    ////////////////////////////////////////////////////
    // Defaults specific to callgrind
    pub const COMPRESS_POS: bool = false;
    pub const COMPRESS_STRINGS: bool = false;
    pub const COMBINE_DUMPS: bool = false;
    pub const DUMP_LINE: bool = true;
    pub const DUMP_INSTR: bool = false;
    pub const SEPARATE_THREADS: bool = true;
    ////////////////////////////////////////////////////

    ////////////////////////////////////////////////////
    // Shared defaults between error emitting tools like Memcheck
    pub const ERROR_EXIT_CODE_ERROR_TOOL: &str = "201";
    pub const ERROR_EXIT_CODE_OTHER_TOOL: &str = "0";
    ////////////////////////////////////////////////////

    ////////////////////////////////////////////////////
    // Shared defaults between all tools
    pub const TRACE_CHILDREN: bool = true;
    pub const FAIR_SCHED: FairSched = FairSched::Try;
    pub const VERBOSE: bool = false;
    pub const VGDB: Vgdb = Vgdb::No;
    ////////////////////////////////////////////////////
}

use std::ffi::OsString;
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use log::warn;
use nix::NixPath;

use super::path::ToolOutputPath;
use crate::api::{RawToolArgs, ValgrindTool};
use crate::error::Error;
use crate::util::{bool_to_yesno, yesno_to_bool};

/// The possible values of the --fair-sched cli arg
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairSched {
    /// Corresponds to `yes`
    Yes,
    /// Corresponds to `no`
    No,
    /// Corresponds to `try`
    Try,
}

/// The possible values for --vgdb
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vgdb {
    /// Corresponds to `yes`
    Yes,
    /// Corresponds to `no`
    No,
    /// Corresponds to `full`
    Full,
}

/// Common parsing behavior for Valgrind tool arguments.
pub trait ToolArgs: Sized {
    /// Try to create new arguments from multiple [`RawToolArgs`].
    fn try_from_raw_tool_args(tool: ValgrindTool, raw_tool_args: &[&RawToolArgs]) -> Result<Self>;

    /// Try to update these arguments from the contents of an iterator.
    fn try_update<'a, T>(&mut self, args: T) -> Result<()>
    where
        T: Iterator<Item = &'a String>;
}

/// The arguments to pass to the Valgrind tool
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValgrindArgs {
    /// The error exit code for error checking tools like `Memcheck`
    pub error_exitcode: String,
    /// The --fair-sched argument
    pub fair_sched: FairSched,
    /// The logfile paths argument --log-file
    pub log_path: Option<OsString>,
    /// All other arguments
    pub other: Vec<String>,
    /// The output paths argument like --callgrind-out-file, ...
    pub output_paths: Vec<OsString>,
    /// The [`ValgrindTool`]
    pub tool: ValgrindTool,
    /// The --trace-children argument
    pub trace_children: bool,
    /// If --verbose is set to true of false
    pub verbose: bool,
    /// The --vgdb argument
    pub vgdb: Vgdb,
    /// The xtree paths argument --xtree-leak-file
    pub xleak_path: Option<OsString>,
    /// The xtree paths argument --xtree-memory-file
    pub xtree_path: Option<OsString>,
}

impl Display for FairSched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Try => "try",
        };
        write!(f, "{string}")
    }
}

impl FromStr for FairSched {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "no" => Ok(Self::No),
            "yes" => Ok(Self::Yes),
            "try" => Ok(Self::Try),
            _ => Err(anyhow!(
                "Invalid argument for --fair-sched. Valid arguments are: 'yes', 'no', 'try'"
            )),
        }
    }
}

impl ToolArgs for ValgrindArgs {
    fn try_from_raw_tool_args(tool: ValgrindTool, raw_tool_args: &[&RawToolArgs]) -> Result<Self> {
        let mut tool_args = Self::new(tool);

        tool_args.try_update(raw_tool_args.iter().flat_map(|args| args.as_slice()))?;

        Ok(tool_args)
    }

    fn try_update<'a, T: Iterator<Item = &'a String>>(&mut self, args: T) -> Result<()> {
        for arg in args {
            let arg = arg.trim();
            match arg.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("--error-exitcode", value)) => {
                    value.clone_into(&mut self.error_exitcode);
                }
                Some((key @ "--trace-children", value)) => {
                    self.trace_children = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some(("--fair-sched", value)) => {
                    self.fair_sched = FairSched::from_str(value)?;
                }
                Some(("--vgdb", value)) => {
                    self.vgdb = Vgdb::from_str(value)?;
                }
                Some((arg, _)) if is_ignored_outfile_argument(arg) => warn!(
                    "Ignoring {} argument '{arg}': Output/Log files of tools are managed by \
                     Gungraun",
                    self.tool.id()
                ),
                Some((arg, _)) if is_ignored_argument(arg) => {
                    warn!("Ignoring {} argument '{arg}'", self.tool.id());
                }
                None if matches!(arg, "-v" | "--verbose") => self.verbose = true,
                None if is_ignored_argument(arg) => {
                    warn!("Ignoring {} argument '{arg}'", self.tool.id());
                }
                None | Some(_) => self.other.push(arg.to_owned()),
            }
        }

        Ok(())
    }
}

impl ValgrindArgs {
    /// Create a new `ValgrindArgs` with the defaults for this tool.
    pub fn new(tool: ValgrindTool) -> Self {
        Self {
            tool,
            output_paths: Vec::default(),
            log_path: Option::default(),
            xtree_path: Option::default(),
            xleak_path: Option::default(),
            error_exitcode: match tool {
                ValgrindTool::Memcheck | ValgrindTool::Helgrind | ValgrindTool::DRD => {
                    defaults::ERROR_EXIT_CODE_ERROR_TOOL.to_owned()
                }
                ValgrindTool::Callgrind
                | ValgrindTool::Massif
                | ValgrindTool::DHAT
                | ValgrindTool::BBV
                | ValgrindTool::Cachegrind => defaults::ERROR_EXIT_CODE_OTHER_TOOL.to_owned(),
            },
            verbose: defaults::VERBOSE,
            other: Vec::default(),
            trace_children: defaults::TRACE_CHILDREN,
            fair_sched: defaults::FAIR_SCHED,
            vgdb: defaults::VGDB,
        }
    }

    /// Set the output file argument depending on the tool of this `ValgrindArgs`
    pub fn set_output_arg(
        &mut self,
        output_path: &ToolOutputPath,
        valgrind_runner_dest: Option<&Path>,
    ) {
        if !self.tool.has_output_file() {
            return;
        }

        match self.tool {
            ValgrindTool::Callgrind => {
                let arg = self.generate_file_arg(
                    "--callgrind-out-file=",
                    output_path,
                    valgrind_runner_dest,
                    None,
                );
                self.output_paths.push(arg);
            }
            ValgrindTool::Massif => {
                let arg = self.generate_file_arg(
                    "--massif-out-file=",
                    output_path,
                    valgrind_runner_dest,
                    None,
                );
                self.output_paths.push(arg);
            }
            ValgrindTool::DHAT => {
                let arg = self.generate_file_arg(
                    "--dhat-out-file=",
                    output_path,
                    valgrind_runner_dest,
                    None,
                );
                self.output_paths.push(arg);
            }
            ValgrindTool::BBV => {
                let bb_arg = self.generate_file_arg(
                    "--bb-out-file=",
                    output_path,
                    valgrind_runner_dest,
                    Some("bb"),
                );
                let pc_arg = self.generate_file_arg(
                    "--pc-out-file=",
                    output_path,
                    valgrind_runner_dest,
                    Some("pc"),
                );
                self.output_paths.push(bb_arg);
                self.output_paths.push(pc_arg);
            }
            ValgrindTool::Cachegrind => {
                let arg = self.generate_file_arg(
                    "--cachegrind-out-file=",
                    output_path,
                    valgrind_runner_dest,
                    None,
                );

                self.output_paths.push(arg);
            }
            // The other tools don't have an outfile
            _ => {}
        }
    }

    /// Set the logfile argument
    pub fn set_log_arg(
        &mut self,
        output_path: &ToolOutputPath,
        valgrind_runner_dest: Option<&Path>,
    ) {
        let arg = self.generate_file_arg(
            "--log-file=",
            &output_path.to_log_output(),
            valgrind_runner_dest,
            None,
        );
        self.log_path = Some(arg);
    }

    /// Set the xtree-memory-file argument for tools which support it
    pub fn set_xtree_arg(
        &mut self,
        output_path: &ToolOutputPath,
        valgrind_runner_dest: Option<&Path>,
    ) {
        if let Some(output_path) = output_path.to_xtree_output() {
            let arg = self.generate_file_arg(
                "--xtree-memory-file=",
                &output_path,
                valgrind_runner_dest,
                None,
            );
            self.xtree_path = Some(arg);
        }
    }

    /// Set the xtree-leak-file argument for tools which support it
    pub fn set_xleak_arg(
        &mut self,
        output_path: &ToolOutputPath,
        valgrind_runner_dest: Option<&Path>,
    ) {
        if let Some(output_path) = output_path.to_xleak_output() {
            let arg = self.generate_file_arg(
                "--xtree-leak-file=",
                &output_path,
                valgrind_runner_dest,
                None,
            );
            self.xleak_path = Some(arg);
        }
    }

    /// Convert into a vector of arguments usable as input for [`std::process::Command::args`]
    pub fn to_vec(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = vec![];

        vec.push(format!("--tool={}", self.tool.id()).into());
        vec.push(format!("--error-exitcode={}", self.error_exitcode).into());
        vec.push(format!("--trace-children={}", bool_to_yesno(self.trace_children)).into());
        vec.push(format!("--fair-sched={}", self.fair_sched).into());
        vec.push(format!("--vgdb={}", self.vgdb).into());
        if self.verbose {
            vec.push("--verbose".into());
        }

        vec.extend(self.other.iter().map(OsString::from));
        vec.extend_from_slice(&self.output_paths);
        if let Some(log_arg) = self.log_path.as_ref() {
            vec.push(log_arg.clone());
        }
        if let Some(xtree_arg) = self.xtree_path.as_ref() {
            vec.push(xtree_arg.clone());
        }
        if let Some(xleak_arg) = self.xleak_path.as_ref() {
            vec.push(xleak_arg.clone());
        }

        vec
    }

    fn generate_file_arg(
        &self,
        arg: &str,
        output_path: &ToolOutputPath,
        valgrind_runner_dest: Option<&Path>,
        extra_modifier: Option<&str>,
    ) -> OsString {
        let output_path = match (self.trace_children, extra_modifier) {
            (true, Some(modifier)) => output_path.with_modifiers([modifier, "#%p"]),
            (true, None) => output_path.with_modifiers(["#%p"]),
            (false, Some(modifier)) => output_path.with_modifiers([modifier, "#0"]),
            (false, None) => output_path.with_modifiers(["#0"]),
        };

        let path = match valgrind_runner_dest {
            Some(dest) => dest.join(output_path.file_name()),
            None => output_path.to_path(),
        };

        let mut file_arg = OsString::with_capacity(arg.len().saturating_add(path.len()));
        file_arg.push(arg);
        file_arg.push(path);
        file_arg
    }
}

impl Display for Vgdb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Full => "full",
        };
        write!(f, "{string}")
    }
}

impl FromStr for Vgdb {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "no" => Ok(Self::No),
            "yes" => Ok(Self::Yes),
            "full" => Ok(Self::Full),
            _ => Err(anyhow!(
                "Invalid argument for --vgdb. Valid arguments are: 'yes', 'no', 'full'"
            )),
        }
    }
}

/// Returns `true` if this is an ignored argument related to output or logfiles.
pub fn is_ignored_outfile_argument(arg: &str) -> bool {
    matches!(
        arg,
        "--dhat-out-file"
            | "--massif-out-file"
            | "--callgrind-out-file"
            | "--cachegrind-out-file"
            | "--bb-out-file"
            | "--pc-out-file"
            | "--log-file"
            | "--log-fd"
            | "--log-socket"
            | "--xml"
            | "--xml-file"
            | "--xml-fd"
            | "--xml-socket"
            | "--xml-user-comment"
            | "--xtree-leak-file"
            | "--xtree-memory-file"
    )
}

/// Returns `true` if this is a generic ignored argument.
pub fn is_ignored_argument(arg: &str) -> bool {
    matches!(
        arg,
        "-h" | "--help"
            | "--help-dyn-options"
            | "--help-debug"
            | "--version"
            | "-q"
            | "--quiet"
            | "--tool"
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use bon::builder;
    use rstest::rstest;

    use super::*;

    fn assert_contains_args<const N: usize>(actual: &[OsString], expected: [&str; N]) {
        for expected in expected {
            assert!(
                actual.iter().any(|arg| arg.to_string_lossy() == expected),
                "expected serialized arg {expected}"
            );
        }
    }

    fn strings<const N: usize>(args: [&str; N]) -> Vec<String> {
        args.into_iter().map(str::to_owned).collect()
    }

    #[builder(finish_fn = "fixture")]
    pub fn valgrind_args_f(
        tool: Option<ValgrindTool>,
        error_exitcode: Option<&str>,
        fair_sched: Option<FairSched>,
        other: Option<Vec<String>>,
        trace_children: Option<bool>,
        verbose: Option<bool>,
        vgdb: Option<Vgdb>,
    ) -> ValgrindArgs {
        let mut args = ValgrindArgs::new(tool.unwrap_or(ValgrindTool::Memcheck));
        if let Some(value) = error_exitcode {
            args.error_exitcode = value.to_owned();
        }
        if let Some(value) = fair_sched {
            args.fair_sched = value;
        }
        if let Some(value) = other {
            args.other.extend(value);
        }
        if let Some(value) = trace_children {
            args.trace_children = value;
        }
        if let Some(value) = verbose {
            args.verbose = value;
        }
        if let Some(value) = vgdb {
            args.vgdb = value;
        }

        args
    }

    #[rstest]
    #[case::error_exitcode(
        &["--error-exitcode=99"],
        valgrind_args_f().error_exitcode("99").fixture()
    )]
    #[case::trace_children(
        &["--trace-children=no"],
        valgrind_args_f().trace_children(false).fixture()
    )]
    #[case::fair_sched(
        &["--fair-sched=no"],
        valgrind_args_f().fair_sched(FairSched::No).fixture()
    )]
    #[case::long_verbose(&["--verbose"], valgrind_args_f().verbose(true).fixture())]
    #[case::short_verbose(&["-v"], valgrind_args_f().verbose(true).fixture())]
    #[case::vgdb(&["--vgdb=yes"], valgrind_args_f().vgdb(Vgdb::Yes).fixture())]
    #[case::vgdb(&["--vgdb=no"], valgrind_args_f().vgdb(Vgdb::No).fixture())]
    #[case::vgdb(&["--vgdb=full"], valgrind_args_f().vgdb(Vgdb::Full).fixture())]
    #[case::outfile_is_ignored(&["--log-file=some"], valgrind_args_f().fixture())]
    #[case::other(
        &["--some-arg=yes"],
        valgrind_args_f()
            .other(strings(["--some-arg=yes"]))
            .fixture()
    )]
    fn test_try_from_raw_tool_args(#[case] args: &[&str], #[case] expected: ValgrindArgs) {
        let actual = ValgrindArgs::try_from_raw_tool_args(
            ValgrindTool::Memcheck,
            &[&RawToolArgs::from_iter(args)],
        )
        .unwrap();

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::trace_children(&["--trace-children=something"])]
    #[case::fair_sched(&["--fair-sched=something"])]
    #[case::vgdb(&["--vgdb=something"])]
    fn test_try_from_raw_tool_args_when_invalid_then_error(#[case] input: &[&str]) {
        ValgrindArgs::try_from_raw_tool_args(
            ValgrindTool::Memcheck,
            &[&RawToolArgs::from_iter(input)],
        )
        .unwrap_err();
    }

    #[test]
    fn test_to_vec() {
        let args = valgrind_args_f()
            .error_exitcode("99")
            .fair_sched(FairSched::No)
            .trace_children(false)
            .fixture();

        let actual = args.to_vec();

        assert_contains_args(
            &actual,
            [
                "--tool=memcheck",
                "--error-exitcode=99",
                "--trace-children=no",
                "--fair-sched=no",
                "--vgdb=no",
            ],
        );
    }

    #[test]
    fn test_to_vec_when_verbose_and_other_args() {
        let args = valgrind_args_f()
            .verbose(true)
            .other(strings(["--some-arg=yes", "--another-some-arg"]))
            .fixture();

        let actual = args.to_vec();

        assert_contains_args(
            &actual,
            ["--verbose", "--some-arg=yes", "--another-some-arg"],
        );
    }
}
