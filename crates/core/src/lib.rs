//! TelemetryBench core: the time base, sample values, and M4 downsampling straight from raw samples.
//!
//! Phase 1 reduces raw data on every viewport query; the pyramid (precomputed summaries) arrives in Phase 2
//! behind the same [`m4`] entry point.

pub mod m4;
pub mod time;
pub mod value;

pub use m4::{Trace, TraceKind, Viewport, m4};
pub use time::{RateTimes, Times, Timestamp};
pub use value::Value;
