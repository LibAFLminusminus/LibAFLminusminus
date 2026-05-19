//! Statistics shared by the various monitors.
//!
//! Hosts the per-client performance stats used by the introspection macros.

/// Per-iteration / per-stage performance statistics.
pub mod perf_stats;

pub use perf_stats::PerfStats;
