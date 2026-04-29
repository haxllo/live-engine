#[cfg(windows)]
use std::fs::OpenOptions;
#[cfg(windows)]
use std::io::{Read, Write};

use livewall_control::{
    Command, CommandEnvelope, ControlError, ControlErrorCode, HealthReport, MonitorStatus,
    PerformanceMode, PlaybackState, Response, ResponseEnvelope, ServiceState, StartupStatus,
    StatusSnapshot, WallpaperKind, WallpaperSummary,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsAppError {
    #[error("control error ({code:?}): {message}")]
    Control {
        code: ControlErrorCode,
        message: String,
    },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("status snapshot missing from response")]
    MissingStatus,
}

impl SettingsAppError {
    fn from_control(error: ControlError) -> Self {
        Self::Control {
            code: error.code,
            message: error.message,
        }
    }
}

pub trait ControlClient {
    fn request_status(&mut self) -> Result<StatusSnapshot, SettingsAppError>;
    fn send_command(
        &mut self,
        command: Command,
    ) -> Result<Option<StatusSnapshot>, SettingsAppError>;
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub struct LiveWallSettingsApp<C: ControlClient> {
    client: C,
    snapshot: StatusSnapshot,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<C: ControlClient> LiveWallSettingsApp<C> {
    pub fn bootstrap(mut client: C) -> Result<Self, SettingsAppError> {
        let snapshot = client.request_status()?;
        Ok(Self { client, snapshot })
    }

    #[must_use]
    pub fn snapshot(&self) -> &StatusSnapshot {
        &self.snapshot
    }

    pub fn refresh(&mut self) -> Result<(), SettingsAppError> {
        self.snapshot = self.client.request_status()?;
        Ok(())
    }

    pub fn set_wallpaper(
        &mut self,
        monitor_id: impl Into<String>,
        wallpaper_id: impl Into<String>,
    ) -> Result<(), SettingsAppError> {
        self.apply_command(Command::SetWallpaper {
            monitor_id: monitor_id.into(),
            wallpaper_id: wallpaper_id.into(),
        })
    }

    pub fn set_wallpaper_for_all(
        &mut self,
        wallpaper_id: impl Into<String>,
    ) -> Result<(), SettingsAppError> {
        self.apply_command(Command::SetWallpaperForAll {
            wallpaper_id: wallpaper_id.into(),
        })
    }

    pub fn apply_next_wallpaper_for_all(&mut self) -> Result<(), SettingsAppError> {
        if self.snapshot.library.is_empty() {
            return Ok(());
        }

        let current = self
            .snapshot
            .monitors
            .iter()
            .find_map(|monitor| monitor.wallpaper_id.as_deref());
        let next_id = match current
            .and_then(|wallpaper_id| self.library_position(wallpaper_id))
            .map(|index| (index + 1) % self.snapshot.library.len())
        {
            Some(next_index) => self.snapshot.library[next_index].wallpaper_id.clone(),
            None => self.snapshot.library[0].wallpaper_id.clone(),
        };

        self.set_wallpaper_for_all(next_id)
    }

    pub fn clear_wallpaper(
        &mut self,
        monitor_id: impl Into<String>,
    ) -> Result<(), SettingsAppError> {
        self.apply_command(Command::ClearWallpaper {
            monitor_id: monitor_id.into(),
        })
    }

    pub fn pause_all(&mut self) -> Result<(), SettingsAppError> {
        self.apply_command(Command::PauseAll)
    }

    pub fn resume_all(&mut self) -> Result<(), SettingsAppError> {
        self.apply_command(Command::ResumeAll)
    }

    pub fn set_performance_mode(&mut self, mode: PerformanceMode) -> Result<(), SettingsAppError> {
        self.apply_command(Command::SetPerformanceMode { mode })
    }

    pub fn set_startup_enabled(&mut self, enabled: bool) -> Result<(), SettingsAppError> {
        self.apply_command(Command::SetStartup { enabled })
    }

    fn library_position(&self, wallpaper_id: &str) -> Option<usize> {
        self.snapshot
            .library
            .iter()
            .position(|wallpaper| wallpaper.wallpaper_id == wallpaper_id)
    }

    fn apply_command(&mut self, command: Command) -> Result<(), SettingsAppError> {
        self.snapshot = self
            .client
            .send_command(command)?
            .ok_or(SettingsAppError::MissingStatus)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryControlClient {
    status: StatusSnapshot,
}

impl InMemoryControlClient {
    #[must_use]
    pub fn new(status: StatusSnapshot) -> Self {
        Self { status }
    }

    fn apply_command(&mut self, command: Command) -> Result<(), ControlError> {
        match command {
            Command::GetStatus => Ok(()),
            Command::SetWallpaper {
                monitor_id,
                wallpaper_id,
            } => {
                let kind = self
                    .status
                    .library
                    .iter()
                    .find(|wallpaper| wallpaper.wallpaper_id == wallpaper_id)
                    .map(|wallpaper| wallpaper.kind)
                    .ok_or_else(|| {
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
                monitor.wallpaper_id = Some(wallpaper_id);
                monitor.wallpaper_kind = Some(kind);
                Ok(())
            }
            Command::SetWallpaperForAll { wallpaper_id } => {
                let kind = self
                    .status
                    .library
                    .iter()
                    .find(|wallpaper| wallpaper.wallpaper_id == wallpaper_id)
                    .map(|wallpaper| wallpaper.kind)
                    .ok_or_else(|| {
                        ControlError::new(
                            ControlErrorCode::WallpaperNotFound,
                            format!("unknown wallpaper `{wallpaper_id}`"),
                        )
                    })?;
                for monitor in &mut self.status.monitors {
                    monitor.wallpaper_id = Some(wallpaper_id.clone());
                    monitor.wallpaper_kind = Some(kind);
                }
                Ok(())
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
                Ok(())
            }
            Command::PauseAll => {
                self.status.playback_state = PlaybackState::Paused;
                for monitor in &mut self.status.monitors {
                    monitor.playback_state = PlaybackState::Paused;
                }
                Ok(())
            }
            Command::ResumeAll => {
                self.status.playback_state = PlaybackState::Running;
                for monitor in &mut self.status.monitors {
                    monitor.playback_state = PlaybackState::Running;
                }
                Ok(())
            }
            Command::SetPerformanceMode { mode } => {
                self.status.performance_mode = mode;
                Ok(())
            }
            Command::SetStartup { enabled } => {
                self.status.startup.enabled = enabled;
                Ok(())
            }
        }
    }
}

impl ControlClient for InMemoryControlClient {
    fn request_status(&mut self) -> Result<StatusSnapshot, SettingsAppError> {
        Ok(self.status.clone())
    }

    fn send_command(
        &mut self,
        command: Command,
    ) -> Result<Option<StatusSnapshot>, SettingsAppError> {
        self.apply_command(command)
            .map_err(SettingsAppError::from_control)?;
        Ok(Some(self.status.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPipeControlClient {
    pipe_path: String,
    next_request_id: u64,
}

impl NamedPipeControlClient {
    #[must_use]
    pub fn new(pipe_path: impl Into<String>) -> Self {
        Self {
            pipe_path: pipe_path.into(),
            next_request_id: 1,
        }
    }

    fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        request_id
    }

    fn send_request(&self, request_json: &str) -> Result<String, SettingsAppError> {
        #[cfg(windows)]
        {
            let mut pipe = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.pipe_path)
                .map_err(|error| {
                    SettingsAppError::Transport(format!(
                        "failed to open named pipe `{}`: {error}",
                        self.pipe_path
                    ))
                })?;

            pipe.write_all(request_json.as_bytes()).map_err(|error| {
                SettingsAppError::Transport(format!(
                    "failed to write request to `{}`: {error}",
                    self.pipe_path
                ))
            })?;
            pipe.flush().map_err(|error| {
                SettingsAppError::Transport(format!(
                    "failed to flush request to `{}`: {error}",
                    self.pipe_path
                ))
            })?;

            let mut response_bytes = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(bytes_read) => response_bytes.extend_from_slice(&chunk[..bytes_read]),
                    Err(error)
                        if is_windows_pipe_end_of_response(&error)
                            && !response_bytes.is_empty() =>
                    {
                        break;
                    }
                    Err(error) => {
                        return Err(SettingsAppError::Transport(format!(
                            "failed to read response from `{}`: {error}",
                            self.pipe_path
                        )));
                    }
                }
            }

            let response_json = String::from_utf8(response_bytes).map_err(|error| {
                SettingsAppError::Transport(format!(
                    "response from `{}` is not valid UTF-8: {error}",
                    self.pipe_path
                ))
            })?;
            if response_json.trim().is_empty() {
                return Err(SettingsAppError::Transport(format!(
                    "service returned an empty response on `{}`",
                    self.pipe_path
                )));
            }
            return Ok(response_json);
        }

        #[cfg(not(windows))]
        {
            let _ = request_json;
            Err(SettingsAppError::Transport(format!(
                "named pipe transport is only available on Windows (`{}`)",
                self.pipe_path
            )))
        }
    }
}

#[cfg(windows)]
fn is_windows_pipe_end_of_response(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(109 | 232 | 233))
}

impl Default for NamedPipeControlClient {
    fn default() -> Self {
        Self::new(r"\\.\pipe\livewall-service")
    }
}

impl ControlClient for NamedPipeControlClient {
    fn request_status(&mut self) -> Result<StatusSnapshot, SettingsAppError> {
        self.send_command(Command::GetStatus)?
            .ok_or(SettingsAppError::MissingStatus)
    }

    fn send_command(
        &mut self,
        command: Command,
    ) -> Result<Option<StatusSnapshot>, SettingsAppError> {
        let request_id = self.next_request_id();
        let request_json = serde_json::to_string(&CommandEnvelope::new(request_id, command))
            .map_err(|error| {
                SettingsAppError::Transport(format!("request serialization failed: {error}"))
            })?;
        let response_json = self.send_request(&request_json)?;
        let response: ResponseEnvelope = serde_json::from_str(&response_json).map_err(|error| {
            SettingsAppError::Transport(format!("response deserialization failed: {error}"))
        })?;

        if response.request_id != request_id {
            return Err(SettingsAppError::Transport(format!(
                "response request_id mismatch: expected {request_id}, received {}",
                response.request_id
            )));
        }

        match response.response {
            Response::Ok { status } => Ok(status),
            Response::Error { error } => Err(SettingsAppError::from_control(error)),
        }
    }
}

#[must_use]
pub fn sample_status_snapshot() -> StatusSnapshot {
    StatusSnapshot {
        health: HealthReport {
            state: ServiceState::Ready,
            message: Some("settings demo status".into()),
            last_error: None,
        },
        playback_state: PlaybackState::Running,
        performance_mode: PerformanceMode::Balanced,
        startup: StartupStatus { enabled: false },
        monitors: vec![
            MonitorStatus {
                monitor_id: "DISPLAY1".into(),
                display_name: "Primary Display".into(),
                is_primary: true,
                wallpaper_id: Some("coast-video".into()),
                wallpaper_kind: Some(WallpaperKind::Video),
                playback_state: PlaybackState::Running,
            },
            MonitorStatus {
                monitor_id: "DISPLAY2".into(),
                display_name: "Side Display".into(),
                is_primary: false,
                wallpaper_id: Some("aurora-scene".into()),
                wallpaper_kind: Some(WallpaperKind::Scene),
                playback_state: PlaybackState::Running,
            },
        ],
        library: vec![
            WallpaperSummary {
                wallpaper_id: "coast-video".into(),
                title: "Coast".into(),
                kind: WallpaperKind::Video,
                preview_path: Some("wallpapers/samples/coast-video/preview.jpg".into()),
            },
            WallpaperSummary {
                wallpaper_id: "aurora-scene".into(),
                title: "Aurora".into(),
                kind: WallpaperKind::Scene,
                preview_path: Some("wallpapers/samples/aurora-scene/preview.jpg".into()),
            },
        ],
        ..StatusSnapshot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryControlClient, LiveWallSettingsApp, sample_status_snapshot};
    use livewall_control::{PerformanceMode, PlaybackState};

    #[test]
    fn bootstrap_loads_library_and_monitor_state() {
        let client = InMemoryControlClient::new(sample_status_snapshot());
        let app = LiveWallSettingsApp::bootstrap(client).expect("settings app should bootstrap");

        assert_eq!(app.snapshot().library.len(), 2);
        assert_eq!(app.snapshot().monitors.len(), 2);
    }

    #[test]
    fn set_wallpaper_updates_monitor_assignment() {
        let client = InMemoryControlClient::new(sample_status_snapshot());
        let mut app =
            LiveWallSettingsApp::bootstrap(client).expect("settings app should bootstrap");

        app.set_wallpaper("DISPLAY2", "coast-video")
            .expect("wallpaper assignment should succeed");

        let monitor = app
            .snapshot()
            .monitors
            .iter()
            .find(|monitor| monitor.monitor_id == "DISPLAY2")
            .expect("DISPLAY2 should exist");
        assert_eq!(monitor.wallpaper_id.as_deref(), Some("coast-video"));
    }

    #[test]
    fn set_performance_mode_updates_snapshot() {
        let client = InMemoryControlClient::new(sample_status_snapshot());
        let mut app =
            LiveWallSettingsApp::bootstrap(client).expect("settings app should bootstrap");

        app.set_performance_mode(PerformanceMode::BatterySaver)
            .expect("mode update should succeed");

        assert_eq!(
            app.snapshot().performance_mode,
            PerformanceMode::BatterySaver
        );
    }

    #[test]
    fn startup_toggle_updates_snapshot() {
        let client = InMemoryControlClient::new(sample_status_snapshot());
        let mut app =
            LiveWallSettingsApp::bootstrap(client).expect("settings app should bootstrap");

        app.refresh().expect("refresh should succeed");
        app.set_startup_enabled(true)
            .expect("startup command should succeed");

        assert!(app.snapshot().startup.enabled);
    }

    #[test]
    fn next_wallpaper_cycles_library_and_pause_resume_work() {
        let client = InMemoryControlClient::new(sample_status_snapshot());
        let mut app =
            LiveWallSettingsApp::bootstrap(client).expect("settings app should bootstrap");

        app.apply_next_wallpaper_for_all()
            .expect("next wallpaper command should succeed");
        assert!(
            app.snapshot()
                .monitors
                .iter()
                .all(|monitor| monitor.wallpaper_id.as_deref() == Some("aurora-scene"))
        );

        app.clear_wallpaper("DISPLAY2")
            .expect("clear wallpaper command should succeed");
        let monitor = app
            .snapshot()
            .monitors
            .iter()
            .find(|monitor| monitor.monitor_id == "DISPLAY2")
            .expect("DISPLAY2 should exist");
        assert_eq!(monitor.wallpaper_id, None);

        app.pause_all().expect("pause should succeed");
        assert_eq!(app.snapshot().playback_state, PlaybackState::Paused);

        app.resume_all().expect("resume should succeed");
        assert_eq!(app.snapshot().playback_state, PlaybackState::Running);
    }
}
