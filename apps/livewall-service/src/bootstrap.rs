use std::time::Duration;

use livewall_control::{Command, MonitorStatus, PlaybackState, StatusSnapshot, WallpaperSummary};
use livewall_desktop::{DesktopError, MonitorInfo, attach_smoke_test, enumerate_monitors};
use livewall_engine::RuntimeCoordinator;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceOptions {
    pub allow_synthetic_monitor: bool,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            allow_synthetic_monitor: true,
        }
    }
}

#[derive(Debug)]
pub struct LiveWallService {
    coordinator: RuntimeCoordinator,
}

#[derive(Debug, Error)]
pub enum ServiceBootstrapError {
    #[error(transparent)]
    Desktop(#[from] DesktopError),
    #[allow(dead_code)]
    #[error("service command failed: {0}")]
    Command(String),
}

impl LiveWallService {
    pub fn bootstrap(
        options: ServiceOptions,
        library: Vec<WallpaperSummary>,
    ) -> Result<Self, ServiceBootstrapError> {
        let monitors = load_monitors(options)?;
        Ok(Self {
            coordinator: RuntimeCoordinator::new(monitors, library),
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> StatusSnapshot {
        self.coordinator.snapshot()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn handle_command(
        &mut self,
        command: Command,
    ) -> Result<StatusSnapshot, ServiceBootstrapError> {
        self.coordinator
            .apply_command(command)
            .map_err(|error| ServiceBootstrapError::Command(error.message))?;
        Ok(self.coordinator.snapshot())
    }
}

pub fn run_desktop_smoke_test() -> Result<(), ServiceBootstrapError> {
    attach_smoke_test(Duration::from_secs(3))?;
    Ok(())
}

fn load_monitors(options: ServiceOptions) -> Result<Vec<MonitorStatus>, ServiceBootstrapError> {
    load_monitors_with(options, enumerate_monitors)
}

fn load_monitors_with(
    options: ServiceOptions,
    enumerate: fn() -> Result<Vec<MonitorInfo>, DesktopError>,
) -> Result<Vec<MonitorStatus>, ServiceBootstrapError> {
    match enumerate() {
        Ok(monitors) => Ok(monitors
            .into_iter()
            .map(|monitor| MonitorStatus {
                monitor_id: monitor.id,
                display_name: monitor.display_name,
                is_primary: monitor.is_primary,
                wallpaper_id: None,
                wallpaper_kind: None,
                playback_state: PlaybackState::Running,
            })
            .collect()),
        Err(DesktopError::UnsupportedPlatform) if options.allow_synthetic_monitor => {
            Ok(vec![synthetic_monitor()])
        }
        Err(error) => Err(ServiceBootstrapError::Desktop(error)),
    }
}

fn synthetic_monitor() -> MonitorStatus {
    MonitorStatus {
        monitor_id: "SIMULATED_DISPLAY1".into(),
        display_name: "Simulated Display".into(),
        is_primary: true,
        wallpaper_id: None,
        wallpaper_kind: None,
        playback_state: PlaybackState::Running,
    }
}

#[cfg(test)]
mod tests {
    use super::{LiveWallService, ServiceOptions, load_monitors_with};
    use livewall_control::{Command, PlaybackState};
    use livewall_desktop::{DesktopError, MonitorInfo};

    #[test]
    fn bootstrap_initializes_with_available_monitors() {
        let service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())
            .expect("service should bootstrap");

        let snapshot = service.snapshot();
        assert!(!snapshot.monitors.is_empty());
    }

    #[test]
    fn load_monitors_uses_synthetic_monitor_when_desktop_is_unavailable() {
        fn unsupported_desktop() -> Result<Vec<MonitorInfo>, DesktopError> {
            Err(DesktopError::UnsupportedPlatform)
        }

        let monitors = load_monitors_with(ServiceOptions::default(), unsupported_desktop)
            .expect("service should synthesize a monitor");

        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].monitor_id, "SIMULATED_DISPLAY1");
    }

    #[test]
    fn pause_command_updates_snapshot_state() {
        let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())
            .expect("service should bootstrap");

        let snapshot = service
            .handle_command(Command::PauseAll)
            .expect("command should succeed");

        assert_eq!(snapshot.playback_state, PlaybackState::Paused);
        assert!(
            snapshot
                .monitors
                .iter()
                .all(|monitor| monitor.playback_state == PlaybackState::Paused)
        );
    }
}
