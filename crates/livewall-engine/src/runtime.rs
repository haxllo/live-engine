use std::collections::HashMap;

use livewall_control::{
    Command, ControlError, ControlErrorCode, Event, HealthReport, MonitorStatus, PerformanceMode,
    PlaybackState, ServiceState, StartupStatus, StatusSnapshot, WallpaperSummary,
};

use crate::policy::{FrameDecision, PolicyState, decide_frame_policy};

pub type RuntimeCoordinatorError = ControlError;

#[derive(Debug, Clone)]
pub struct RuntimeCoordinator {
    status: StatusSnapshot,
}

impl RuntimeCoordinator {
    #[must_use]
    pub fn new(monitors: Vec<MonitorStatus>, library: Vec<WallpaperSummary>) -> Self {
        Self {
            status: StatusSnapshot {
                health: HealthReport {
                    state: ServiceState::Ready,
                    message: Some("runtime initialized".into()),
                    last_error: None,
                },
                playback_state: PlaybackState::Running,
                performance_mode: PerformanceMode::Balanced,
                startup: StartupStatus { enabled: false },
                monitors,
                library,
                ..StatusSnapshot::default()
            },
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> StatusSnapshot {
        self.status.clone()
    }

    pub fn apply_command(
        &mut self,
        command: Command,
    ) -> Result<Option<Event>, RuntimeCoordinatorError> {
        match command {
            Command::GetStatus => Ok(Some(Event::StatusPublished {
                status: self.snapshot(),
            })),
            Command::SetWallpaper {
                monitor_id,
                wallpaper_id,
            } => {
                let wallpaper = self.find_wallpaper(&wallpaper_id).cloned().ok_or_else(|| {
                    ControlError::new(
                        ControlErrorCode::WallpaperNotFound,
                        format!("unknown wallpaper `{wallpaper_id}`"),
                    )
                })?;
                let monitor = self
                    .status
                    .monitors
                    .iter_mut()
                    .find(|monitor| monitor.monitor_id == monitor_id)
                    .ok_or_else(|| {
                        ControlError::new(
                            ControlErrorCode::MonitorNotFound,
                            format!("unknown monitor `{monitor_id}`"),
                        )
                    })?;
                monitor.wallpaper_id = Some(wallpaper.wallpaper_id.clone());
                monitor.wallpaper_kind = Some(wallpaper.kind);
                Ok(Some(Event::WallpaperAssigned {
                    monitor_id,
                    wallpaper_id: wallpaper.wallpaper_id,
                }))
            }
            Command::SetWallpaperForAll { wallpaper_id } => {
                let wallpaper = self.find_wallpaper(&wallpaper_id).cloned().ok_or_else(|| {
                    ControlError::new(
                        ControlErrorCode::WallpaperNotFound,
                        format!("unknown wallpaper `{wallpaper_id}`"),
                    )
                })?;
                for monitor in &mut self.status.monitors {
                    monitor.wallpaper_id = Some(wallpaper.wallpaper_id.clone());
                    monitor.wallpaper_kind = Some(wallpaper.kind);
                }
                Ok(Some(Event::StatusPublished {
                    status: self.snapshot(),
                }))
            }
            Command::ClearWallpaper { monitor_id } => {
                let monitor = self
                    .status
                    .monitors
                    .iter_mut()
                    .find(|monitor| monitor.monitor_id == monitor_id)
                    .ok_or_else(|| {
                        ControlError::new(
                            ControlErrorCode::MonitorNotFound,
                            format!("unknown monitor `{monitor_id}`"),
                        )
                    })?;
                monitor.wallpaper_id = None;
                monitor.wallpaper_kind = None;
                Ok(Some(Event::StatusPublished {
                    status: self.snapshot(),
                }))
            }
            Command::PauseAll => {
                self.status.playback_state = PlaybackState::Paused;
                for monitor in &mut self.status.monitors {
                    monitor.playback_state = PlaybackState::Paused;
                }
                Ok(Some(Event::PlaybackStateChanged {
                    playback_state: PlaybackState::Paused,
                }))
            }
            Command::ResumeAll => {
                self.status.playback_state = PlaybackState::Running;
                for monitor in &mut self.status.monitors {
                    monitor.playback_state = PlaybackState::Running;
                }
                Ok(Some(Event::PlaybackStateChanged {
                    playback_state: PlaybackState::Running,
                }))
            }
            Command::SetPerformanceMode { mode } => {
                self.status.performance_mode = mode;
                Ok(Some(Event::PerformanceModeChanged { mode }))
            }
            Command::SetStartup { enabled } => {
                self.status.startup.enabled = enabled;
                Ok(Some(Event::StartupStatusChanged { enabled }))
            }
        }
    }

    #[must_use]
    pub fn apply_policy_state(&mut self, state: &PolicyState) -> FrameDecision {
        let decision = decide_frame_policy(state, self.status.performance_mode);
        self.status.playback_state = decision.playback_state;
        for monitor in &mut self.status.monitors {
            monitor.playback_state = decision.playback_state;
        }
        decision
    }

    pub fn replace_monitors(&mut self, monitors: Vec<MonitorStatus>) {
        let existing_by_id: HashMap<_, _> = self
            .status
            .monitors
            .iter()
            .map(|monitor| (monitor.monitor_id.clone(), monitor.clone()))
            .collect();

        self.status.monitors = monitors
            .into_iter()
            .map(|mut monitor| {
                if let Some(existing) = existing_by_id.get(&monitor.monitor_id) {
                    monitor.wallpaper_id = existing.wallpaper_id.clone();
                    monitor.wallpaper_kind = existing.wallpaper_kind;
                    monitor.playback_state = existing.playback_state;
                }
                monitor
            })
            .collect();
    }

    fn find_wallpaper(&self, wallpaper_id: &str) -> Option<&WallpaperSummary> {
        self.status
            .library
            .iter()
            .find(|wallpaper| wallpaper.wallpaper_id == wallpaper_id)
    }
}
