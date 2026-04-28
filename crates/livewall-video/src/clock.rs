use std::time::Duration;

use livewall_pack::LoopMode;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClockState {
    Playing,
    #[default]
    Paused,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSchedule {
    pub fps: f32,
    pub frame_interval: Duration,
}

impl FrameSchedule {
    #[must_use]
    pub fn new(fps: f32) -> Self {
        let clamped_fps = fps.max(1.0);
        let frame_interval_nanos = (1_000_000_000f64 / f64::from(clamped_fps)).round() as u64;
        Self {
            fps: clamped_fps,
            frame_interval: Duration::from_nanos(frame_interval_nanos),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackSnapshot {
    pub state: ClockState,
    pub position: Duration,
    pub wrapped: bool,
    pub next_frame_deadline: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackClock {
    duration: Duration,
    loop_mode: LoopMode,
    frame_schedule: FrameSchedule,
    state: ClockState,
    position: Duration,
    last_update: Duration,
}

impl PlaybackClock {
    #[must_use]
    pub fn new(duration: Duration, loop_mode: LoopMode, fps: f32) -> Self {
        Self {
            duration,
            loop_mode,
            frame_schedule: FrameSchedule::new(fps),
            state: ClockState::Paused,
            position: Duration::ZERO,
            last_update: Duration::ZERO,
        }
    }

    #[must_use]
    pub fn state(&self) -> ClockState {
        self.state
    }

    #[must_use]
    pub fn position(&self) -> Duration {
        self.position
    }

    #[must_use]
    pub fn frame_schedule(&self) -> FrameSchedule {
        self.frame_schedule
    }

    pub fn play(&mut self, now: Duration) {
        self.state = ClockState::Playing;
        self.last_update = now;
    }

    pub fn pause(&mut self, now: Duration) {
        let _ = self.update(now);
        self.state = ClockState::Paused;
        self.last_update = now;
    }

    pub fn seek(&mut self, position: Duration, now: Duration) {
        self.position = position.min(self.duration);
        self.last_update = now;
        if self.position >= self.duration && self.loop_mode == LoopMode::Once {
            self.state = ClockState::Ended;
        }
    }

    pub fn update(&mut self, now: Duration) -> PlaybackSnapshot {
        let mut wrapped = false;
        if self.state == ClockState::Playing {
            let delta = now.saturating_sub(self.last_update);
            self.position += delta;
            self.last_update = now;

            if self.position >= self.duration {
                match self.loop_mode {
                    LoopMode::Loop if !self.duration.is_zero() => {
                        let total = self.position.as_nanos();
                        let span = self.duration.as_nanos().max(1);
                        self.position = duration_from_nanos((total % span) as u64);
                        wrapped = true;
                    }
                    LoopMode::Loop => {
                        self.position = Duration::ZERO;
                        wrapped = true;
                    }
                    LoopMode::Once => {
                        self.position = self.duration;
                        self.state = ClockState::Ended;
                    }
                }
            }
        } else {
            self.last_update = now;
        }

        PlaybackSnapshot {
            state: self.state,
            position: self.position,
            wrapped,
            next_frame_deadline: self.next_frame_deadline(now),
        }
    }

    fn next_frame_deadline(&self, now: Duration) -> Option<Duration> {
        if self.state != ClockState::Playing {
            return None;
        }

        Some(now + self.frame_schedule.frame_interval)
    }
}

fn duration_from_nanos(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}
