//! TODO: DOCS

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::v6;

/// TODO: DOCS
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SummaryByVersion {
    /// TODO: DOCS
    V6(v6::BenchmarkSummary),
}

/// TODO: DOCS
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Version {
    /// TODO: DOCS
    V6,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct VersionProbe {
    version: String,
}

impl SummaryByVersion {
    /// TODO: DOCS
    pub fn version(&self) -> Version {
        match self {
            Self::V6(_) => Version::V6,
        }
    }
}

impl Version {
    /// TODO: DOCS
    pub const fn as_str(&self) -> &str {
        match self {
            Self::V6 => v6::SCHEMA_VERSION,
        }
    }
}

/// TODO: DOCS
pub fn parse(path: &Path) -> Result<SummaryByVersion> {
    fs::read(path)
        .map_err(|error| Error::ParseError(format!("'{}': {error}", path.display())))
        .and_then(|buffer| parse_slice(&buffer))
}

/// TODO: DOCS
pub fn parse_slice(buffer: &[u8]) -> Result<SummaryByVersion> {
    let probe: VersionProbe =
        serde_json::from_slice(buffer).map_err(|error| Error::ParseError(error.to_string()))?;

    match probe.version.as_str() {
        v6::SCHEMA_VERSION => v6::parse_slice(buffer).map(SummaryByVersion::V6),
        version => Err(Error::UnsupportedVersion(version.to_owned())),
    }
}
