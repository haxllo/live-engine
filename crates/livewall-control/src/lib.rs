//! Shared IPC contracts for the LiveWall service and settings app.
//!
//! Compatibility contract:
//! - Clients send `CommandEnvelope` values with `protocol_version` set to
//!   [`PROTOCOL_VERSION`].
//! - Services reject mismatched versions and return a typed
//!   [`ControlErrorCode::ProtocolVersionMismatch`] through [`Response::Error`].

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

pub type RequestId = u64;

#[must_use]
pub const fn is_protocol_compatible(version: u32) -> bool {
    version == PROTOCOL_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub protocol_version: u32,
    pub request_id: RequestId,
    pub command: Command,
}

impl CommandEnvelope {
    #[must_use]
    pub fn new(request_id: RequestId, command: Command) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            command,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub protocol_version: u32,
    pub request_id: RequestId,
    pub response: Response,
}

impl ResponseEnvelope {
    #[must_use]
    pub fn ok(request_id: RequestId, status: Option<StatusSnapshot>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response: Response::Ok { status },
        }
    }

    #[must_use]
    pub fn error(request_id: RequestId, error: ControlError) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response: Response::Error { error },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub protocol_version: u32,
    pub event: Event,
}

impl EventEnvelope {
    #[must_use]
    pub fn new(event: Event) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            event,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum Command {
    GetStatus,
    SetWallpaper {
        monitor_id: String,
        wallpaper_id: String,
    },
    SetWallpaperForAll {
        wallpaper_id: String,
    },
    ClearWallpaper {
        monitor_id: String,
    },
    PauseAll,
    ResumeAll,
    SetPerformanceMode {
        mode: PerformanceMode,
    },
    SetStartup {
        enabled: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum Response {
    Ok { status: Option<StatusSnapshot> },
    Error { error: ControlError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum Event {
    StatusPublished {
        status: StatusSnapshot,
    },
    WallpaperAssigned {
        monitor_id: String,
        wallpaper_id: String,
    },
    PlaybackStateChanged {
        playback_state: PlaybackState,
    },
    PerformanceModeChanged {
        mode: PerformanceMode,
    },
    StartupStatusChanged {
        enabled: bool,
    },
    HealthChanged {
        health: HealthReport,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceMode {
    Quality,
    #[default]
    Balanced,
    BatterySaver,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Running,
    #[default]
    Paused,
    Suspended,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    #[default]
    Starting,
    Ready,
    Degraded,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperKind {
    Video,
    Scene,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub protocol_version: u32,
    pub health: HealthReport,
    pub playback_state: PlaybackState,
    pub performance_mode: PerformanceMode,
    pub startup: StartupStatus,
    pub monitors: Vec<MonitorStatus>,
    pub library: Vec<WallpaperSummary>,
}

impl Default for StatusSnapshot {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            health: HealthReport::default(),
            playback_state: PlaybackState::default(),
            performance_mode: PerformanceMode::default(),
            startup: StartupStatus::default(),
            monitors: Vec::new(),
            library: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    pub state: ServiceState,
    pub message: Option<String>,
    pub last_error: Option<ControlError>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupStatus {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorStatus {
    pub monitor_id: String,
    pub display_name: String,
    pub is_primary: bool,
    pub wallpaper_id: Option<String>,
    pub wallpaper_kind: Option<WallpaperKind>,
    pub playback_state: PlaybackState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperSummary {
    pub wallpaper_id: String,
    pub title: String,
    pub kind: WallpaperKind,
    pub preview_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlError {
    pub code: ControlErrorCode,
    pub message: String,
    pub expected_protocol_version: Option<u32>,
    pub actual_protocol_version: Option<u32>,
}

impl ControlError {
    #[must_use]
    pub fn protocol_version_mismatch(actual_protocol_version: u32) -> Self {
        Self {
            code: ControlErrorCode::ProtocolVersionMismatch,
            message: format!(
                "protocol version {actual_protocol_version} is not supported; expected {PROTOCOL_VERSION}"
            ),
            expected_protocol_version: Some(PROTOCOL_VERSION),
            actual_protocol_version: Some(actual_protocol_version),
        }
    }

    #[must_use]
    pub fn new(code: ControlErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            expected_protocol_version: None,
            actual_protocol_version: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlErrorCode {
    ProtocolVersionMismatch,
    InvalidCommand,
    MonitorNotFound,
    WallpaperNotFound,
    ServiceUnavailable,
    Conflict,
}
