//! Metric value and comparison types used inside parsed Gungraun summaries.
//!
//! These types describe raw metric values, per-metric diffs, and grouped metric summaries that
//! appear throughout the version 6 summary model.

use std::cmp::Ordering;
use std::hash::Hash;

use either_or_both::EitherOrBoth;
use indexmap::IndexMap;
#[cfg(feature = "summary")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::{CachegrindMetric, DhatMetric, ErrorMetric, EventKind};
use crate::summary::model::Diffs;

/// The value type used for metrics measured by a benchmark tool
///
/// Raw metrics emitted by Valgrind tools are [`Metric::Int`] which is the default metric type.
/// Metrics that have [`Metric::Float`] type are documented as such. Derived values, such as miss
/// rates and hit rates, require floating-point representation. `Metric` preserves both forms in the
/// parsed summary model.
///
/// # Developer Notes
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

/// Identifies a metric kind by tool
///
/// This enum appears in places where a summary needs to describe a metric without separately
/// carrying the tool family that owns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub enum MetricKind {
    /// The `None` kind if there are no metrics for a tool (i.e. BBV and Massif)
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

/// An insertion-ordered mapping from metric identifier to [`Metric`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct Metrics<K: Hash + Eq>(pub IndexMap<K, Metric>);

/// Comparison data for one metric in a parsed summary.
///
/// If both, old and new values, are present, [`Diffs`] stores the derived percentage and factor.
/// Otherwise the summary only stores whichever side is available. Per convention, the left side or
/// [`EitherOrBoth::Left`] stores the new [`Metric`] and the right side or [`EitherOrBoth::Right`]
/// stores the old metric.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "summary", derive(JsonSchema))]
pub struct MetricsDiff {
    /// If both metrics ([`EitherOrBoth::Both`]) are present there is also a `Diffs` present
    pub diffs: Option<Diffs>,
    /// Either the `new` ([`EitherOrBoth::Left`]), `old` ([`EitherOrBoth::Right`]) or both metrics
    pub metrics: EitherOrBoth<Metric>,
}

/// An insertion-ordered mapping from metric identifier to [`MetricsDiff`].
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
