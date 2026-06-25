//! TODO: DOCS

use std::fs;
use std::path::Path;

pub use gungraun_runner::summary::model::*;

use crate::error::{Error, Result};

/// TODO: DOCS
pub fn parse(path: &Path) -> Result<BenchmarkSummary> {
    fs::read(path)
        .map_err(|error| Error::ParseError(format!("'{}': {error}", path.display())))
        .and_then(|buffer| parse_slice(&buffer))
}

/// TODO: DOCS
pub fn parse_slice(buffer: &[u8]) -> Result<BenchmarkSummary> {
    serde_json::from_slice(buffer).map_err(|error| Error::ParseError(error.to_string()))
}
