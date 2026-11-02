//! TelemetryBench core: the time base, sample values, M4 downsampling, and the min/max pyramid that answers views
//! of any span from a few thousand summaries. Pure algorithms: files and caches live in `tlmbench-store`.

pub mod datetime;
pub mod gaps;
pub mod m4;
pub mod pyramid;
pub mod recording;
pub mod time;
pub mod value;
pub mod wire;

pub use m4::{Trace, TraceKind, Viewport, m4, m4_breaking};
pub use recording::{ChannelInfo, Recording, RecordingInfo, TimeSource};
pub use time::{RateTimes, Times, Timestamp};
pub use value::{Samples, Value};
