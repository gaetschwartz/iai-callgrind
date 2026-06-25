//! TODO: DOCS

use std::cmp::Ordering;
use std::hash::Hash;

use either_or_both::EitherOrBoth;
use indexmap::IndexMap;
#[cfg(feature = "summary")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind};
use crate::summary::model::Diffs;

/// The metric measured by valgrind or derived from one or more other metrics
///
/// The valgrind metrics measured by any of its tools are `u64`. However, to be able to represent
/// derived metrics like cache miss/hit rates it is inevitable to have a type which can store a
/// `u64` or a `f64`. When doing math with metrics, the original type should be preserved as far as
/// possible by using `u64` operations. A float metric should be a last resort.
///
/// Float operations with a `Metric` that stores a `u64` introduce a precision loss and are to be
/// avoided. Especially comparison between a `u64` metric and `f64` metric are not exact because the
/// `u64` has to be converted to a `f64`. Also, if adding/multiplying two `u64` metrics would result
/// in an overflow the metric saturates at `u64::MAX`. This choice was made to preserve precision
/// and the original type (instead of for example adding the two `u64` by converting both of them to
/// `f64`).
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum Metric {
    /// An integer `Metric`
    Int(u64),
    /// A float `Metric`
    Float(f64),
}

/// The different metrics distinguished by tool and if it is an error checking tool as `ErrorMetric`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum MetricKind {
    /// The `None` kind if there are no metrics for a tool
    None,
    /// The Callgrind metric kind
    Callgrind(EventKind),
    /// The Cachegrind metric kind
    Cachegrind(CachegrindMetric),
    /// The DHAT metric kind
    Dhat(DhatMetric),
    /// The Memcheck metric kind
    Memcheck(ErrorMetric),
    /// The Helgrind metric kind
    Helgrind(ErrorMetric),
    /// The DRD metric kind
    DRD(ErrorMetric),
}

/// The `Metrics` backed by an [`indexmap::IndexMap`]
///
/// The insertion order is preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct Metrics<K: Hash + Eq>(pub IndexMap<K, Metric>);

/// The `MetricsDiff` describes the difference between a `new` and `old` metric as percentage and
/// factor.
///
/// Only if both metrics are present there is also a `Diffs` present. Otherwise, it just stores the
/// `new` or `old` metric.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct MetricsDiff {
    /// If both metrics are present there is also a `Diffs` present
    pub diffs: Option<Diffs>,
    /// Either the `new`, `old` or both metrics
    pub metrics: EitherOrBoth<Metric>,
}

/// The `MetricsSummary` contains all differences between two tool run segments
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct MetricsSummary<K: Hash + Eq = EventKind>(pub IndexMap<K, MetricsDiff>);

impl Eq for Metric {}

impl Ord for Metric {
    #[expect(clippy::cast_precision_loss)]
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b),
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)),
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b),
        }
    }
}

impl PartialOrd for Metric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Metric {
    #[expect(clippy::cast_precision_loss)]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b) == Ordering::Equal,
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)) == Ordering::Equal,
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b) == Ordering::Equal,
        }
    }
}
