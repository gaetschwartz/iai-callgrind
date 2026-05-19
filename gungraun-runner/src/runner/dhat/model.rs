//! This module contains the structs to model the dhat output file content

use std::ops::Sub;
// spell-checker: ignore bklt bkacc bksu tuth ftbl tgmax
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use simplematch::DoWild;

use crate::api::EntryPoint;
use crate::runner::DEFAULT_TOGGLE;
use crate::runner::dhat::tree::Data;

static FRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?<root>\[root\])|(?<addr>0x[0-9a-fA-F]+):\s*(?<func>.*)\s\((?<in>.*)\)$")
        .expect("Regex should compile")
});

/// A [`Frame`] in the [`DhatData::frame_table`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// The root frame
    Root,
    /// All other frames than the root are leafs
    Leaf(String, String, String),
}

/// The dhat invocation mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// --mode=heap
    #[default]
    Heap,
    /// --mode=ad-hoc
    AdHoc,
    /// --mode=copy
    Copy,
}

/// The top-level data extracted from dhat json output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(clippy::arbitrary_source_item_ordering)]
pub struct DhatData {
    /// Top-level metadata
    #[serde(flatten)]
    pub metadata: DhatMetadata,

    /// The [`ProgramPoint`]s
    #[serde(rename = "pps")]
    pub program_points: Vec<ProgramPoint>,

    /// [`Frame`] table
    #[serde(rename = "ftbl")]
    pub frame_table: Vec<Frame>,
}

/// Top-level metadata extracted from dhat json output.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(clippy::arbitrary_source_item_ordering)]
pub struct DhatMetadata {
    /// Version number of the format
    #[serde(rename = "dhatFileVersion")]
    pub dhat_file_version: usize,

    /// The invocation mode
    pub mode: Mode,

    /// The verb used before above stack frames
    pub verb: String,

    /// Are block lifetimes recorded? Affects whether some other fields are present.
    #[serde(rename = "bklt")]
    pub has_block_lifetimes: bool,

    /// Are block accesses recorded? Affects whether some other fields are present
    #[serde(rename = "bkacc")]
    pub has_block_accesses: bool,

    /// Byte units. "byte" is the values used if these fields are omitted.
    #[serde(rename = "bu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_unit: Option<String>,

    /// Bytes units. "bytes" is the values used if these fields are omitted.
    #[serde(rename = "bsu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_unit: Option<String>,

    /// Blocks units. "blocks" is the values used if these fields are omitted.
    #[serde(rename = "bksu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_unit: Option<String>,

    /// Time units (individual)
    #[serde(rename = "tu")]
    pub time_unit: String,

    /// Time units (1,000,000x)
    #[serde(rename = "Mtu")]
    pub time_unit_m: String,

    /// The "short-lived" time threshold, measures in "tu"s (`time_unit`).
    /// - bklt=true: a mandatory integer.
    /// - bklt=false: omitted.
    #[serde(rename = "tuth")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_threshold: Option<usize>,

    /// The executed command
    #[serde(rename = "cmd")]
    pub command: String,

    /// The process ID
    pub pid: i32,

    /// The time at the end of execution (t-end)
    #[serde(rename = "te")]
    pub time_end: u64,

    /// The time of the global max (t-gmax)
    /// - bklt=true: a mandatory integer.
    /// - bklt=false: omitted.
    #[serde(rename = "tg")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_global_max: Option<u64>,
}

/// A `ProgramPoint`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[expect(clippy::arbitrary_source_item_ordering)]
pub struct ProgramPoint {
    /// Total bytes
    #[serde(rename = "tb")]
    pub total_bytes: u64,

    /// Total blocks
    #[serde(rename = "tbk")]
    pub total_blocks: u64,

    /// Total lifetimes of all blocks allocated at this `ProgramPoint`.
    /// - bklt=true: a mandatory integer.
    /// - bklt=false: omitted.
    #[serde(rename = "tl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_lifetimes: Option<u64>,

    /// The maximum bytes for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "mb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_bytes: Option<u64>,

    /// The maximum blocks for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "mbk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_blocks: Option<u64>,

    /// The bytes at t-gmax for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "gb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_at_max: Option<u64>,

    /// The blocks at t-gmax for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "gbk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_at_max: Option<u64>,

    /// The bytes at t-end for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "eb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_at_end: Option<u64>,

    /// The blocks at t-end for this `ProgramPoint`
    /// - bklt=true: mandatory integers.
    /// - bklt=false: omitted.
    #[serde(rename = "ebk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_at_end: Option<u64>,

    /// The reads of blocks for this `ProgramPoint`
    /// - bkacc=true: mandatory integers.
    /// - bkacc=false: omitted.
    #[serde(rename = "rb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_read: Option<u64>,

    /// The writes of blocks for this `ProgramPoint`
    /// - bkacc=true: mandatory integers.
    /// - bkacc=false: omitted.
    #[serde(rename = "wb")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_write: Option<u64>,

    /// The exact accesses of blocks for this `ProgramPoint`. Only used when all allocations are
    /// the same size and sufficiently small. A negative element indicates run-length encoding
    /// of the following integer. E.g. `-3, 4` means "three 4s in a row".
    /// - bkacc=true: an optional array of integers.
    /// - bkacc=false: omitted.
    #[serde(rename = "acc")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accesses: Option<Vec<i64>>,

    /// Frames. Each element is an index into the [`DhatData::frame_table`]
    #[serde(rename = "fs")]
    pub frames: Vec<usize>,
}

impl DhatData {
    /// TODO: DOCS
    pub fn with_metadata(metadata: DhatMetadata) -> Self {
        Self {
            metadata,
            program_points: Vec::default(),
            frame_table: Vec::default(),
        }
    }

    /// TODO: DOCS
    pub fn filter_program_points(&mut self, entry_point: &EntryPoint, frames: &[String]) {
        let mut globs = frames.iter().collect::<Vec<_>>();

        let glob = match entry_point {
            EntryPoint::None => None,
            EntryPoint::Default => Some(DEFAULT_TOGGLE.into()),
            EntryPoint::Custom(custom) => Some(custom.into()),
        };

        if let Some(glob) = &glob {
            globs.push(glob);
        }

        let mut indices = vec![];
        if !globs.is_empty() {
            for (index, frame) in self.frame_table.iter().enumerate() {
                if let Frame::Leaf(_, func_name, _) = frame {
                    for glob in &globs {
                        if glob.as_str().dowild(func_name) {
                            indices.push(index);
                        }
                    }
                }
            }
        }

        if *entry_point == EntryPoint::None && frames.is_empty() {
            // do nothing
        } else if !indices.is_empty() {
            self.program_points = self
                .program_points
                .iter()
                .filter(|p| p.frames.iter().any(|f| indices.contains(f)))
                .cloned()
                .collect();
        } else {
            self.program_points = Vec::default();
        }
    }

    /// TODO: DOCS
    pub fn sanitize(&mut self, mapping_table: &[usize]) {
        if self.frame_table.is_empty() {
            self.frame_table.insert(0, Frame::Root);
        } else {
            for program_point in &mut self.program_points {
                for frame in &mut program_point.frames {
                    *frame = mapping_table[*frame];
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let frame = String::deserialize(deserializer)?;
        Self::from_str(&frame).map_err(Error::custom)
    }
}

impl From<(&str, &str, &str)> for Frame {
    fn from((addr, func, loc): (&str, &str, &str)) -> Self {
        Self::Leaf(addr.to_owned(), func.to_owned(), loc.to_owned())
    }
}

impl FromStr for Frame {
    type Err = String;

    fn from_str(haystack: &str) -> Result<Self, Self::Err> {
        let caps = FRAME_RE
            .captures(haystack)
            .ok_or_else(|| "invalid frame format".to_owned())?;

        if caps.name("root").is_some() {
            Ok(Self::Root)
        } else {
            Ok(Self::Leaf(
                caps.name("addr")
                    .expect("An address should be present")
                    .as_str()
                    .to_owned(),
                caps.name("func")
                    .expect("A function should be present")
                    .as_str()
                    .to_owned(),
                caps.name("in")
                    .expect("A location should be present")
                    .as_str()
                    .to_owned(),
            ))
        }
    }
}

impl ProgramPoint {
    /// TODO: DOCS
    pub fn is_zero(&self) -> bool {
        self.total_bytes == 0
            && self.total_blocks == 0
            && self.total_lifetimes.is_none_or(|value| value == 0)
            && self.maximum_bytes.is_none_or(|value| value == 0)
            && self.maximum_blocks.is_none_or(|value| value == 0)
            && self.bytes_at_max.is_none_or(|value| value == 0)
            && self.blocks_at_max.is_none_or(|value| value == 0)
            && self.bytes_at_end.is_none_or(|value| value == 0)
            && self.blocks_at_end.is_none_or(|value| value == 0)
            && self.blocks_read.is_none_or(|value| value == 0)
            && self.blocks_write.is_none_or(|value| value == 0)
    }

    /// TODO: DOCS
    pub fn sub(&mut self, data: &Data) {
        self.total_bytes -= data.total_bytes;
        self.total_blocks -= data.total_blocks;
        self.total_lifetimes = sub_options(self.total_lifetimes, data.total_lifetimes);
        self.maximum_bytes = sub_options(self.maximum_bytes, data.maximum_bytes);
        self.maximum_blocks = sub_options(self.maximum_blocks, data.maximum_blocks);
        self.bytes_at_max = sub_options(self.bytes_at_max, data.bytes_at_max);
        self.blocks_at_max = sub_options(self.blocks_at_max, data.blocks_at_max);
        self.bytes_at_end = sub_options(self.bytes_at_end, data.bytes_at_end);
        self.blocks_at_end = sub_options(self.blocks_at_end, data.blocks_at_end);
        self.blocks_read = sub_options(self.blocks_read, data.blocks_read);
        self.blocks_write = sub_options(self.blocks_write, data.blocks_write);
    }
}
impl Serialize for Frame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = match self {
            Self::Root => "[root]".to_owned(),
            Self::Leaf(addr, func, loc) => format!("{addr}: {func} ({loc})"),
        };

        serializer.serialize_str(&string)
    }
}

impl<'de> Deserialize<'de> for Mode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let frame = String::deserialize(deserializer)?;
        let mode = match frame.to_lowercase().as_str() {
            "ad-hoc" => Self::AdHoc,
            "heap" => Self::Heap,
            "copy" => Self::Copy,
            _ => return Err(Error::custom("Invalid mode")),
        };

        Ok(mode)
    }
}

impl Serialize for Mode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = match self {
            Self::Heap => "heap",
            Self::AdHoc => "ad-hoc",
            Self::Copy => "copy",
        };

        serializer.serialize_str(string)
    }
}

fn sub_options<T: Sub<Output = T>>(lhs: Option<T>, rhs: Option<T>) -> Option<T> {
    match (lhs, rhs) {
        (Some(a), None) => Some(a),
        (Some(a), Some(b)) => Some(a - b),
        // For our representation, The case None - Some(x) is safer to be handled as None. Usually,
        // we don't hit this case because either both options are Some or None when extracted from
        // the original dhat data.
        (None, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_test::{Token, assert_tokens};

    use super::*;

    #[rstest]
    #[case::short_addr(
        "0x1234: malloc (in /usr/lib/some.so)", ("0x1234", "malloc", "in /usr/lib/some.so")
    )]
    #[case::no_in(
        "0x12345678: malloc (/usr/lib/some.so)", ("0x12345678", "malloc", "/usr/lib/some.so")
    )]
    #[case::some(
        "0x12345678: malloc (in /usr/lib/some.so)", ("0x12345678", "malloc", "in /usr/lib/some.so")
    )]
    #[case::long_with_multiple_parentheses(
    "0x40440E3: call_once<(), (dyn core::ops::function::Fn<(), Output=i32> + core::marker::Sync + \
    core::panic::unwind_safe::RefUnwindSafe)> (function.rs:284)",
    (
        "0x40440E3",
        "call_once<(), (dyn core::ops::function::Fn<(), Output=i32> + \
        core::marker::Sync + core::panic::unwind_safe::RefUnwindSafe)>",
        "function.rs:284"
    )
)]
    fn test_frame_from_str(#[case] haystack: &str, #[case] frame: (&str, &str, &str)) {
        let expected = Frame::from(frame);
        let actual = haystack.parse::<Frame>().unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_frame_de_and_serialize_frame() {
        let frame = Frame::from(("0x1234", "malloc", "in /usr/lib/some.so"));
        assert_tokens(
            &frame,
            &[Token::Str("0x1234: malloc (in /usr/lib/some.so)")],
        );
    }

    #[test]
    fn test_frame_de_and_serialize_root() {
        let frame = Frame::Root;
        assert_tokens(&frame, &[Token::Str("[root]")]);
    }

    #[test]
    fn test_frame_from_str_when_root() {
        let expected = Frame::Root;
        let actual = "[root]".parse::<Frame>().unwrap();
        assert_eq!(actual, expected);
    }
}
