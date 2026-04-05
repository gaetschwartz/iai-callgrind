//! The module containing the [`Metadata`] and [`Cmd`]

// spell-checker: ignore beforemidafter startend

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};
use cargo_metadata::TargetKind;
use clap::Parser;
use log::debug;

use super::args::CommandLineArgs;
use super::envs;
use crate::api::ValgrindTool;
use crate::runner::args::{self, RawArgs};
use crate::runner::tool::config::ToolConfig;
use crate::runner::tool::path::ToolOutputPath;
use crate::runner::tool::run::RunOptions;
use crate::util::{bool_to_yesno, resolve_binary_path};

/// TODO: DOCS, cannot use `std::process::Command` directly because it cannot be cloned
#[derive(Debug, Clone)]
pub enum ValgrindRunMode {
    /// TODO: DOCS
    DisabledASLR(Cmd),
    /// TODO: DOCS
    Valgrind(Cmd),
    /// TODO: DOCS
    ValgrindRunner(PathBuf, PathBuf),
}

/// The basic commands (like valgrind) to be executed with default arguments
#[derive(Debug, Clone)]
pub struct Cmd {
    /// The arguments for the executable
    pub args: Vec<OsString>,
    /// The path to the executable
    pub bin: PathBuf,
}

/// `Metadata` contains all information that needs to be collected from cargo and the environment
///
/// More specifically, `Metadata` contains global constants, environment variables and command-line
/// arguments, the basic valgrind [`Cmd`], ...
#[derive(Debug, Clone)]
pub struct Metadata {
    /// A string describing the architecture of the CPU that is currently in use (e.g. "x86")
    pub arch: String,
    /// The command-line arguments parsed from the arguments to `cargo bench -- ARGS` as ARGS
    pub args: CommandLineArgs,
    /// The name of the benchmark to run (might be different to the name of the file)
    pub bench_name: String,
    /// The path to the project top-level directory
    pub project_root: PathBuf,
    /// The absolute path of the `HOME` (per default `$WORKSPACE_ROOT/target/gungraun`). Plus, if
    /// configured, the target of the host like `x86_64-linux-unknown-gnu`. The final component is
    /// the `CARGO_PKG_NAME`.
    ///
    /// Examples:
    /// * `/home/my/workspace/my-project/target/gungraun/my-project` or
    /// * `/home/my/workspace/my-project/target/gungraun/x86_64-linux-unknown-gnu/my-project`
    pub target_dir: PathBuf,
    /// TODO: DOCS
    pub valgrind_run_mode: ValgrindRunMode,
}

impl Metadata {
    /// Create a `new` Metadata
    pub fn new(
        raw_command_line_args: &[String],
        package_name: &str,
        bench_file: &Path,
    ) -> Result<Self> {
        let args = CommandLineArgs::parse_from(raw_command_line_args);

        let arch = std::env::consts::ARCH.to_owned();
        debug!("Detected architecture: {arch}");

        let meta = cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .expect("Querying metadata of cargo workspace succeeds");

        let package = meta
            .packages
            .iter()
            .find(|p| p.name == package_name)
            .expect("The package name should exist");
        let bench_name = package
            .targets
            .iter()
            .find_map(|t| {
                (t.kind.contains(&TargetKind::Bench) && t.src_path.ends_with(bench_file))
                    .then_some(t.name.clone())
            })
            .expect("The benchmark name should exist");

        let project_root = meta.workspace_root.into_std_path_buf();
        debug!("Detected workspace root: '{}'", project_root.display());

        let target_dir = {
            let mut home = args.home.as_ref().map_or_else(
                || {
                    std::env::var_os(envs::CARGO_TARGET_DIR)
                        .map_or_else(|| meta.target_directory.into_std_path_buf(), PathBuf::from)
                        .join("gungraun")
                },
                Clone::clone,
            );

            if args.separate_targets {
                home = home.join(env!("GR_BUILD_TRIPLE").to_ascii_lowercase());
            }
            home.join(
                std::env::var_os(envs::CARGO_PKG_NAME).map_or_else(PathBuf::new, PathBuf::from),
            )
        };

        debug!("Detected target directory: '{}'", target_dir.display());

        let valgrind_path = args
            .valgrind_path
            .clone()
            .or_else(|| resolve_binary_path("valgrind", None).ok())
            .unwrap_or_else(|| PathBuf::from("valgrind"));

        debug!("Detected valgrind path: '{}'", valgrind_path.display());

        let valgrind_run_mode = if let Some(runner) = args.valgrind_runner.as_ref() {
            let resolved = resolve_binary_path(runner, None)?;
            debug!("Using valgrind runner: '{}'", resolved.display());

            ValgrindRunMode::ValgrindRunner(resolved, valgrind_path)
        } else {
            // Invoke Valgrind, disabling ASLR if possible because ASLR could noise up the results a
            // bit
            let valgrind_wrapper = if args.allow_aslr.unwrap_or(args::defaults::ALLOW_ASLR) {
                debug!("Running with ASLR enabled");
                None
            } else if cfg!(target_os = "linux") {
                debug!("Trying to run with ASLR disabled: Using 'setarch'");

                if let Ok(set_arch) = resolve_binary_path("setarch", None) {
                    Some(Cmd {
                        bin: set_arch,
                        args: vec![
                            OsString::from(&arch),
                            OsString::from("-R"),
                            OsString::from(&valgrind_path),
                        ],
                    })
                } else {
                    debug!(
                        "Failed to switch ASLR off: 'setarch' not found. Running with ASLR enabled"
                    );
                    None
                }
            } else if cfg!(target_os = "freebsd") {
                debug!("Trying to run with ASLR disabled: Using 'proccontrol'");

                if let Ok(proc_control) = resolve_binary_path("proccontrol", None) {
                    Some(Cmd {
                        bin: proc_control,
                        args: vec![
                            OsString::from("-m"),
                            OsString::from("aslr"),
                            OsString::from("-s"),
                            OsString::from("disable"),
                            OsString::from(&valgrind_path),
                        ],
                    })
                } else {
                    debug!(
                        " Failed to switch ASLR off: 'proccontrol' not found. Running with ASLR \
                         enabled"
                    );
                    None
                }
            } else {
                debug!(
                    "Failed to switch ASLR off. No utility available. Running with ASLR enabled"
                );
                None
            };

            valgrind_wrapper.map_or_else(
                || {
                    ValgrindRunMode::Valgrind(Cmd {
                        args: Vec::default(),
                        bin: valgrind_path,
                    })
                },
                ValgrindRunMode::DisabledASLR,
            )
        };

        Ok(Self {
            arch,
            args,
            bench_name,
            project_root,
            target_dir,
            valgrind_run_mode,
        })
    }

    /// TODO: DOCS
    pub fn to_tool_command(
        &self,
        tool_config: &ToolConfig,
        output_path: &ToolOutputPath,
        run_options: &RunOptions,
    ) -> Result<Command> {
        match &self.valgrind_run_mode {
            ValgrindRunMode::DisabledASLR(cmd) | ValgrindRunMode::Valgrind(cmd) => {
                let mut command = Command::new(&cmd.bin);

                if run_options.env_clear {
                    debug!("Clearing environment variables");
                    env_clear(tool_config.tool, &mut command);
                }

                command.args(&cmd.args);
                command.envs(&run_options.envs);
                Ok(command)
            }
            ValgrindRunMode::ValgrindRunner(runner_path, valgrind_path) => {
                let mut command = Command::new(runner_path);
                let mut additional_envs = HashMap::new();

                additional_envs.insert(
                    OsString::from("GUNGRAUN_VR_DEST_DIR"),
                    OsString::from(output_path.dest_dir()),
                );
                additional_envs.insert(
                    OsString::from("GUNGRAUN_VR_HOME"),
                    self.target_dir.clone().into_os_string(),
                );
                additional_envs.insert(
                    OsString::from("GUNGRAUN_VR_WORKSPACE_ROOT"),
                    self.project_root.clone().into_os_string(),
                );
                additional_envs.insert(
                    OsString::from("GUNGRAUN_ALLOW_ASLR"),
                    self.args
                        .allow_aslr
                        .map_or_else(|| bool_to_yesno(args::defaults::ALLOW_ASLR), bool_to_yesno)
                        .into(),
                );

                let mut has_args = false;
                for args in self
                    .args
                    .valgrind_runner_args
                    .iter()
                    .filter(|&r| !r.is_empty())
                    .map(RawArgs::as_slice)
                {
                    has_args = true;
                    let interpolated =
                        interpolate_arguments(args, &run_options.envs, &additional_envs)?;
                    command.args(interpolated);
                }

                if has_args {
                    command.arg("--");
                }

                command.arg(valgrind_path);

                if run_options.env_clear {
                    debug!("Clearing environment variables");
                    env_clear(tool_config.tool, &mut command);
                }

                // `additional_envs` are added before the run options envs so the user can overwrite
                // them if required
                command.envs(additional_envs);
                command.envs(&run_options.envs);

                Ok(command)
            }
        }
    }
}

fn interpolate_arguments(
    args: &[String],
    envs: &HashMap<OsString, OsString>,
    additional_envs: &HashMap<OsString, OsString>,
) -> Result<Vec<OsString>> {
    args.iter()
        .map(|arg| interpolate_argument(arg, envs, additional_envs))
        .collect::<Result<_>>()
}

fn interpolate_argument(
    arg: &str,
    envs: &HashMap<OsString, OsString>,
    additional_envs: &HashMap<OsString, OsString>,
) -> Result<OsString> {
    let mut result = Vec::with_capacity(arg.len());
    let chars = arg.as_bytes();
    let mut index = 0;

    while index < chars.len() {
        let char = chars[index];

        let next_index = index + 1;
        if next_index < chars.len() {
            let next = chars[next_index];
            match (char, next) {
                (b'$', b'{') if next_index + 1 < chars.len() => {
                    let dollar_pos = index;
                    let mut is_valid = false;
                    let start = next_index + 1;
                    let mut end = 0;
                    index = next_index + 1;
                    for c in &chars[start..] {
                        end = index;
                        index += 1;

                        if *c == b'}' {
                            is_valid = end > start;
                            break;
                        }
                    }

                    if is_valid {
                        // SAFETY: The input arg is a `&str` and valid UTF-8 so everything within
                        // `${...}` must be valid UTF-8, too.
                        let var = unsafe {
                            OsStr::from_encoded_bytes_unchecked(&arg.as_bytes()[start..end])
                        };
                        let value = additional_envs
                            .get(var)
                            .cloned()
                            .or_else(|| envs.get(var).cloned())
                            .or_else(|| std::env::var_os(var))
                            .ok_or_else(|| {
                                anyhow!(
                                    "Failed to interpolate the variable '{}' at column \
                                     '{dollar_pos}': Variable not found in the environment",
                                    var.to_string_lossy()
                                )
                            })?;

                        result.append(&mut value.into_encoded_bytes());
                    } else {
                        return Err(anyhow!(
                            "Failed to interpolate the variable at column '{dollar_pos}': Invalid \
                             syntax"
                        ));
                    }
                }
                (b'$', b'{') => {
                    return Err(anyhow!(
                        "Failed to interpolate the variable at column '{index}': Premature end of \
                         variable declaration"
                    ))
                }
                (char, b'$') if next_index + 1 < chars.len() => {
                    result.push(char);

                    index += 1;
                }
                (a, b) => {
                    result.push(a);
                    result.push(b);

                    index += 2;
                }
            }
        } else {
            result.push(char);
            index += 1;
        }
    }

    // SAFETY: The result bytes vector is a mixture of valid `UTF-8` from &arg which is a `&str` and
    // the values of environment variables which are valid `OsStrings` producing together a valid
    // encoding.
    Ok(unsafe { OsString::from_encoded_bytes_unchecked(result) })
}

/// Clear the environment variables
///
/// The `LD_PRELOAD` and `LD_LIBRARY_PATH` variables are skipped. If they are set there's
/// usually a good reason for it.
///
/// If the tool is `Memcheck`: In order to be able run `Memcheck` without errors, the `PATH`,
/// `HOME` and `DEBUGINFOD_URLS` variables are skipped.
pub fn env_clear(tool: ValgrindTool, command: &mut Command) {
    debug!("{}: Clearing environment variables", tool.id());
    for (key, _) in std::env::vars() {
        match (key.as_str(), tool) {
                (key @ ("DEBUGINFOD_URLS" | "PATH" | "HOME"), ValgrindTool::Memcheck)
                // FIX: (Remove all of them? or just VALGRIND_LIB) but also provide --envs,
                // --passthrough-envs
                | (key @ ("LD_PRELOAD" | "LD_LIBRARY_PATH" | "VALGRIND_LIB"), _) => {
                    debug!(
                        "{}: Clearing environment variables: Skipping {key}",
                        tool.id()
                    );
                }
                _ => {
                    command.env_remove(key);
                }
            }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;

    use rstest::rstest;

    use super::*;

    fn make_envs(pairs: &[(&str, &str)]) -> HashMap<OsString, OsString> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    #[rstest]
    #[case::single_var("${VAR}", make_envs(&[("VAR", "value")]), make_envs(&[]), "value")]
    #[case::multiple_vars("${A}${B}", make_envs(&[("A", "1"), ("B", "2")]), make_envs(&[]), "12")]
    #[case::var_with_text(
            "prefix_${VAR}_suffix",
            make_envs(&[("VAR", "value")]),
            make_envs(&[]),
            "prefix_value_suffix"
        )]
    #[case::var_middle("before${MID}after", make_envs(&[("MID", "mid")]), make_envs(&[]), "beforemidafter")]
    #[case::var_at_start("${START}end", make_envs(&[("START", "start")]), make_envs(&[]), "startend")]
    #[case::var_at_end("start${END}", make_envs(&[("END", "end")]), make_envs(&[]), "startend")]
    #[case::empty_string("", make_envs(&[]), make_envs(&[]), "")]
    #[case::no_vars("plain text", make_envs(&[]), make_envs(&[]), "plain text")]
    #[case::additional_envs_priority(
            "${VAR}",
            make_envs(&[("VAR", "from_envs")]),
            make_envs(&[("VAR", "from_additional")]),
            "from_additional"
        )]
    #[case::envs_over_real_env("${VAR}", make_envs(&[("VAR", "from_envs")]), make_envs(&[]), "from_envs")]
    #[serial_test::serial]
    fn test_interpolate_argument_basic(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_interpolate_argument_std_env_is_used() {
        const VAR_NAME: &str = "GUNGRAUN_TEST_INTERPOLATE_VAR";
        std::env::set_var(VAR_NAME, "from_real_env");
        let envs = HashMap::new();
        let additional_envs = HashMap::new();
        let result = interpolate_argument(&format!("${{{VAR_NAME}}}"), &envs, &additional_envs);
        std::env::remove_var(VAR_NAME);
        assert_eq!(result.unwrap(), OsString::from("from_real_env"));
    }

    #[rstest]
    #[case::utf8_in_value("${VAR}", make_envs(&[("VAR", "日本語")]), make_envs(&[]), "日本語")]
    #[case::utf8_in_text("日本${VAR}語", make_envs(&[("VAR", "-")]), make_envs(&[]), "日本-語")]
    #[case::space_in_value("${VAR}", make_envs(&[("VAR", "hello world")]), make_envs(&[]), "hello world")]
    #[case::special_chars("${VAR}", make_envs(&[("VAR", "--flag=value")]), make_envs(&[]), "--flag=value")]
    #[case::path_separators(
            "${VAR}",
            make_envs(&[("VAR", "/usr/local/bin")]),
            make_envs(&[]),
            "/usr/local/bin"
        )]
    #[serial_test::serial]
    fn test_interpolate_argument_special_chars(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[rstest]
    #[case::same_var_thrice("${A}${A}${A}", make_envs(&[("A", "x")]), make_envs(&[]), "xxx")]
    #[case::same_var_with_text("${A}_${A}", make_envs(&[("A", "val")]), make_envs(&[]), "val_val")]
    #[serial_test::serial]
    fn test_interpolate_argument_same_var(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[rstest]
    #[case::dollar_only("$", make_envs(&[]), make_envs(&[]), "$")]
    #[case::double_dollar("$$", make_envs(&[]), make_envs(&[]), "$$")]
    #[case::dollar_before_text("$abc", make_envs(&[]), make_envs(&[]), "$abc")]
    #[case::dollar_before_brace("$} text", make_envs(&[]), make_envs(&[]), "$} text")]
    #[serial_test::serial]
    fn test_interpolate_argument_literal_dollar(
        #[case] arg: &str,
        #[case] envs: HashMap<OsString, OsString>,
        #[case] additional_envs: HashMap<OsString, OsString>,
        #[case] expected: &str,
    ) {
        assert_eq!(
            interpolate_argument(arg, &envs, &additional_envs).unwrap(),
            OsString::from(expected)
        );
    }

    #[rstest]
    #[case::empty_var_name(
        "${}",
        "Failed to interpolate the variable at column '0': Invalid syntax"
    )]
    #[case::unclosed_var(
        "${VAR",
        "Failed to interpolate the variable at column '0': Invalid syntax"
    )]
    #[case::var_not_found(
        "${NOTFOUND}",
        "Failed to interpolate the variable 'NOTFOUND' at column '0': Variable not found in the \
         environment"
    )]
    #[serial_test::serial]
    fn test_interpolate_argument_when_error(#[case] arg: &str, #[case] expected_error: &str) {
        let envs = HashMap::new();
        let additional_envs = HashMap::new();
        let err = interpolate_argument(arg, &envs, &additional_envs).unwrap_err();
        assert_eq!(err.to_string(), expected_error);
    }

    #[rstest]
    #[case::empty_slice(&[], make_envs(&[]), vec![])]
    #[case::single_arg(&["${VAR}"], make_envs(&[("VAR", "val")]), vec!["val"])]
    #[case::multiple_args(&["${A}", "${B}"], make_envs(&[("A", "1"), ("B", "2")]), vec!["1", "2"])]
    #[case::mixed(&["plain", "${VAR}", "text"], make_envs(&[("VAR", "x")]), vec!["plain", "x", "text"])]
    #[serial_test::serial]
    fn test_interpolate_arguments(
        #[case] args: &[&str],
        #[case] envs: HashMap<OsString, OsString>,
        #[case] expected: Vec<&str>,
    ) {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        let result = interpolate_arguments(&args, &envs, &HashMap::new()).unwrap();
        assert_eq!(
            result,
            expected.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }
}
