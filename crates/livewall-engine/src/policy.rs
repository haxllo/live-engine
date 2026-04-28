use livewall_control::{PerformanceMode, PlaybackState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyState {
    pub user_paused: bool,
    pub fullscreen_app: bool,
    pub display_sleeping: bool,
    pub on_battery: bool,
    pub battery_percent: Option<u8>,
    pub source_fps: u32,
}

impl Default for PolicyState {
    fn default() -> Self {
        Self {
            user_paused: false,
            fullscreen_app: false,
            display_sleeping: false,
            on_battery: false,
            battery_percent: None,
            source_fps: 60,
        }
    }
}

impl PolicyState {
    #[must_use]
    pub fn with_fullscreen_app(mut self, fullscreen_app: bool) -> Self {
        self.fullscreen_app = fullscreen_app;
        self
    }

    #[must_use]
    pub fn with_on_battery(mut self, on_battery: bool) -> Self {
        self.on_battery = on_battery;
        self
    }

    #[must_use]
    pub fn with_battery_percent(mut self, battery_percent: Option<u8>) -> Self {
        self.battery_percent = battery_percent;
        self
    }

    #[must_use]
    pub fn with_display_sleeping(mut self, display_sleeping: bool) -> Self {
        self.display_sleeping = display_sleeping;
        self
    }

    #[must_use]
    pub fn with_source_fps(mut self, source_fps: u32) -> Self {
        self.source_fps = source_fps.max(1);
        self
    }

    #[must_use]
    pub fn with_user_paused(mut self, user_paused: bool) -> Self {
        self.user_paused = user_paused;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDecision {
    pub playback_state: PlaybackState,
    pub target_fps: u32,
    pub decode_allowed: bool,
}

#[must_use]
pub fn decide_frame_policy(state: &PolicyState, mode: PerformanceMode) -> FrameDecision {
    if state.display_sleeping || state.fullscreen_app || state.user_paused {
        return FrameDecision {
            playback_state: PlaybackState::Paused,
            target_fps: 0,
            decode_allowed: false,
        };
    }

    match mode {
        PerformanceMode::Quality => FrameDecision {
            playback_state: PlaybackState::Running,
            target_fps: state.source_fps.max(1),
            decode_allowed: true,
        },
        PerformanceMode::Balanced => FrameDecision {
            playback_state: PlaybackState::Running,
            target_fps: if state.on_battery {
                state.source_fps.min(24)
            } else {
                state.source_fps.min(30)
            }
            .max(1),
            decode_allowed: true,
        },
        PerformanceMode::BatterySaver => {
            if matches!(state.battery_percent, Some(percent) if percent <= 10) {
                FrameDecision {
                    playback_state: PlaybackState::Paused,
                    target_fps: 0,
                    decode_allowed: false,
                }
            } else {
                FrameDecision {
                    playback_state: PlaybackState::Running,
                    target_fps: state.source_fps.clamp(1, 24),
                    decode_allowed: true,
                }
            }
        }
    }
}
