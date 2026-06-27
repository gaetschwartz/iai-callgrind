//! The gungraun-runner library

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(test(attr(warn(unused))))]
#![doc(test(attr(allow(unused_extern_crates))))]

#[cfg(any(feature = "api", feature = "summary", feature = "schema"))]
pub mod api;
#[cfg(feature = "runner")]
pub mod error;
#[cfg(any(feature = "runner", feature = "summary", feature = "schema"))]
pub mod metrics;
#[cfg(feature = "runner")]
pub mod runner;
#[cfg(any(feature = "api", feature = "summary", feature = "schema"))]
pub mod serde;
#[cfg(any(feature = "runner", feature = "summary", feature = "schema"))]
pub mod summary;
#[cfg(feature = "runner")]
pub mod util;
