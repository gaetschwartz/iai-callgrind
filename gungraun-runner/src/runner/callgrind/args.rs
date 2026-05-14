//! The module containing the command line arguments for callgrind
use std::collections::VecDeque;

use anyhow::Result;
use log::warn;

use crate::api::{RawToolArgs, ValgrindTool};
use crate::error::Error;
use crate::runner::tool::args::{ToolArgs, ValgrindArgs, defaults};
use crate::util::{bool_to_yesno, yesno_to_bool};

/// The command-line arguments
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct CallgrindArgs {
    cache_sim: bool,
    /// --combine-dumps is currently not supported by the Callgrind parsers, so we print a warning
    combine_dumps: bool,
    compress_pos: bool,
    compress_strings: bool,
    d1: String,
    dump_instr: bool,
    dump_line: bool,
    i1: String,
    ll: String,
    separate_threads: bool,
    toggle_collect: VecDeque<String>,
    valgrind_args: ValgrindArgs,
}

impl ToolArgs for CallgrindArgs {
    fn try_from_raw_tool_args(tool: ValgrindTool, raw_tool_args: &[&RawToolArgs]) -> Result<Self> {
        debug_assert_eq!(tool, ValgrindTool::Cachegrind);

        let mut default = Self::default();
        default.try_update(raw_tool_args.iter().flat_map(|s| s.as_slice()))?;
        Ok(default)
    }

    fn try_update<'a, T: Iterator<Item = &'a String>>(&mut self, args: T) -> Result<()> {
        for arg in args {
            let trimmed = arg.trim();
            match trimmed.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("--I1", value)) => value.clone_into(&mut self.i1),
                Some(("--D1", value)) => value.clone_into(&mut self.d1),
                Some(("--LL", value)) => value.clone_into(&mut self.ll),
                Some((key @ "--cache-sim", value)) => {
                    self.cache_sim = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some(("--toggle-collect", value)) => {
                    self.toggle_collect.push_back(value.to_owned());
                }
                Some((key @ "--dump-instr", value)) => {
                    self.dump_instr = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some((key @ "--dump-line", value)) => {
                    self.dump_line = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some((key @ "--separate-threads", value)) => {
                    self.separate_threads = yesno_to_bool(value).ok_or_else(|| {
                        Error::InvalidBoolArgument(key.to_owned(), value.to_owned())
                    })?;
                }
                Some((
                    key @ ("--combine-dumps" | "--compress-strings" | "--compress-pos"),
                    value,
                )) => {
                    warn!("Ignoring unsupported callgrind argument: '{key}={value}'");
                }
                None | Some(_) => self.valgrind_args.try_update(std::iter::once(arg))?,
            }
        }
        Ok(())
    }
}

impl Default for CallgrindArgs {
    fn default() -> Self {
        Self {
            // Set some reasonable cache sizes. The exact sizes matter less than having fixed sizes,
            // since otherwise callgrind would take them from the CPU and make benchmark runs
            // even more incomparable between machines.
            i1: defaults::I1.into(),
            d1: defaults::D1.into(),
            ll: defaults::LL.into(),
            cache_sim: defaults::CACHE_SIM,
            compress_pos: defaults::COMPRESS_POS,
            compress_strings: defaults::COMPRESS_STRINGS,
            combine_dumps: defaults::COMBINE_DUMPS,
            dump_line: defaults::DUMP_LINE,
            dump_instr: defaults::DUMP_INSTR,
            toggle_collect: VecDeque::default(),
            separate_threads: defaults::SEPARATE_THREADS,
            valgrind_args: ValgrindArgs::new(ValgrindTool::Callgrind),
        }
    }
}

impl From<CallgrindArgs> for ValgrindArgs {
    fn from(value: CallgrindArgs) -> Self {
        let mut valgrind = value.valgrind_args;
        let other = vec![
            format!("--I1={}", &value.i1),
            format!("--D1={}", &value.d1),
            format!("--LL={}", &value.ll),
            format!("--cache-sim={}", bool_to_yesno(value.cache_sim)),
            format!(
                "--compress-strings={}",
                bool_to_yesno(value.compress_strings)
            ),
            format!("--compress-pos={}", bool_to_yesno(value.compress_pos)),
            format!("--dump-line={}", bool_to_yesno(value.dump_line)),
            format!("--dump-instr={}", bool_to_yesno(value.dump_instr)),
            format!("--combine-dumps={}", bool_to_yesno(value.combine_dumps)),
            format!(
                "--separate-threads={}",
                bool_to_yesno(value.separate_threads)
            ),
        ];
        valgrind.other.extend(other);
        valgrind.other.extend(
            value
                .toggle_collect
                .into_iter()
                .map(|s| format!("--toggle-collect={s}")),
        );

        valgrind
    }
}
