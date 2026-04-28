//! Video descriptor loading and playback clock utilities.

mod clock;
mod player;

pub use clock::{ClockState, FrameSchedule, PlaybackClock, PlaybackSnapshot};
pub use player::{VideoDescriptor, VideoPlayer, VideoPlayerError, load_video_descriptor};
