//! TelemetryBench core: the time base, sample values, M4 downsampling, and the min/max pyramid that answers views
//! of any span from a few thousand summaries. Pure algorithms: files and caches live in `tlmbench-store`.

pub mod chunks;
pub mod datetime;
pub mod gaps;
pub mod m4;
pub mod mask;
pub mod pyramid;
pub mod recording;
pub mod table;
pub mod time;
pub mod value;
pub mod wire;

pub use m4::{Trace, TraceKind, Viewport, m4, m4_breaking};
pub use recording::{ChannelInfo, ChannelOp, Channels, Recording, RecordingInfo, TimeSource, TimesOp};
pub use time::{Indexed, RateTimes, TimeIndex, Times, Timestamp};
pub use value::{Column, Samples, Shifted, Value};
