//! Fixtures for types from the api module

use bon::builder;

use crate::api::{
    CachegrindMetrics, CachegrindRegressionConfig, DhatMetrics, DhatRegressionConfig, Limit,
};

#[builder(finish_fn = "fixture")]
pub fn cachegrind_regression_config_f(
    soft_limits: Option<Vec<(CachegrindMetrics, f64)>>,
    hard_limits: Option<Vec<(CachegrindMetrics, Limit)>>,
    fail_fast: Option<bool>,
) -> CachegrindRegressionConfig {
    CachegrindRegressionConfig {
        soft_limits: soft_limits.map_or_else(Vec::default, |s| s.into_iter().collect()),
        hard_limits: hard_limits.map_or_else(Vec::default, |h| h.into_iter().collect()),
        fail_fast,
    }
}

#[builder(finish_fn = "fixture")]
pub fn dhat_regression_config_f(
    soft_limits: Option<Vec<(DhatMetrics, f64)>>,
    hard_limits: Option<Vec<(DhatMetrics, Limit)>>,
    fail_fast: Option<bool>,
) -> DhatRegressionConfig {
    DhatRegressionConfig {
        soft_limits: soft_limits.map_or_else(Vec::default, |s| s.into_iter().collect()),
        hard_limits: hard_limits.map_or_else(Vec::default, |h| h.into_iter().collect()),
        fail_fast,
    }
}
