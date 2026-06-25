//! TODO: DOCS

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// TODO: DOCS
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// TODO: DOCS
    #[error("error parsing summary: {0}")]
    ParseError(String),
    /// TODO: DOCS
    #[error("failed parsing summary: unsupported version '{0}'")]
    UnsupportedVersion(String),
}

/// TODO: DOCS
pub type Result<T> = std::result::Result<T, Error>;
