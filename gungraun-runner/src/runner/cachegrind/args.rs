//! The module containing the command line arguments for Cachegrind

use anyhow::Result;

use crate::api::{RawToolArgs, ValgrindTool};
use crate::error::Error;
use crate::runner::tool::args::{ToolArgs, ValgrindArgs, defaults};
use crate::util::{bool_to_yesno, yesno_to_bool};

/// The command-line arguments
#[derive(Debug, Clone)]
pub struct CachegrindArgs {
    cache_sim: bool,
    d1: String,
    i1: String,
    ll: String,
    valgrind: ValgrindArgs,
}

impl ToolArgs for CachegrindArgs {
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
                None | Some(_) => self.valgrind.try_update(std::iter::once(arg))?,
            }
        }
        Ok(())
    }
}

impl Default for CachegrindArgs {
    fn default() -> Self {
        Self {
            i1: defaults::I1.into(),
            d1: defaults::D1.into(),
            ll: defaults::LL.into(),
            cache_sim: defaults::CACHE_SIM,
            valgrind: ValgrindArgs::new(ValgrindTool::Cachegrind),
        }
    }
}

impl From<CachegrindArgs> for ValgrindArgs {
    fn from(value: CachegrindArgs) -> Self {
        let mut valgrind = value.valgrind;
        let other = vec![
            format!("--I1={}", &value.i1),
            format!("--D1={}", &value.d1),
            format!("--LL={}", &value.ll),
            format!("--cache-sim={}", bool_to_yesno(value.cache_sim)),
        ];
        valgrind.other.extend(other);

        valgrind
    }
}
