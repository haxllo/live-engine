use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use livewall_control::{
    Command, CommandEnvelope, ControlError, ControlErrorCode, MonitorStatus, PlaybackState,
    ResponseEnvelope, StatusSnapshot, WallpaperSummary, is_protocol_compatible,
};
use livewall_desktop::{
    DesktopError, DesktopHost, MonitorInfo, attach_smoke_test, attach_wallpaper_hosts,
    enumerate_monitors,
};
use livewall_engine::RuntimeCoordinator;
use livewall_render::{RenderDevice, RenderError, create_shared_device};
use thiserror::Error;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, RPC_E_CHANGED_MODE};
#[cfg(windows)]
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFShutdown, MFStartup};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
#[cfg(windows)]
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
#[cfg(windows)]
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
#[cfg(windows)]
use windows::core::PCWSTR;

pub const DEFAULT_PIPE_PATH: &str = r"\\.\pipe\livewall-service";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceOptions {
    pub allow_synthetic_monitor: bool,
    pub pipe_path: String,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            allow_synthetic_monitor: true,
            pipe_path: DEFAULT_PIPE_PATH.into(),
        }
    }
}

#[derive(Debug)]
pub struct LiveWallService {
    coordinator: RuntimeCoordinator,
    runtime: ServiceRuntime,
}

#[derive(Debug)]
struct ServiceRuntime {
    _desktop_host: Option<DesktopHost>,
    _render_device: Option<RenderDevice>,
    _media_runtime: Option<MediaRuntime>,
    ipc_server: ServiceIpcServer,
    logger: ServiceLogger,
}

#[derive(Debug, Error)]
pub enum ServiceBootstrapError {
    #[error(transparent)]
    Desktop(#[from] DesktopError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("control error ({code:?}): {message}")]
    Control {
        code: ControlErrorCode,
        message: String,
    },
    #[error("IPC server configuration error: {0}")]
    IpcConfig(String),
    #[error("{context} failed: {message}")]
    Platform {
        context: &'static str,
        message: String,
    },
}

impl LiveWallService {
    pub fn bootstrap(
        options: ServiceOptions,
        library: Vec<WallpaperSummary>,
    ) -> Result<Self, ServiceBootstrapError> {
        let mut logger = ServiceLogger::open()?;
        logger.info("startup", "service bootstrap started")?;

        let media_runtime = match MediaRuntime::initialize() {
            Ok(runtime) => {
                logger.info("startup", "COM and Media Foundation initialized")?;
                Some(runtime)
            }
            Err(error) if options.allow_synthetic_monitor && error.is_unsupported_platform() => {
                logger.info(
                    "startup",
                    "COM/Media Foundation unavailable on this platform; continuing in degraded mode",
                )?;
                None
            }
            Err(error) => return Err(error),
        };

        let render_device = match create_shared_device() {
            Ok(device) => {
                logger.info("startup", "D3D11 shared device initialized")?;
                Some(device)
            }
            Err(RenderError::Platform { .. }) if options.allow_synthetic_monitor => {
                logger.info(
                    "startup",
                    "D3D11 initialization failed; continuing in degraded mode",
                )?;
                None
            }
            Err(RenderError::UnsupportedPlatform) if options.allow_synthetic_monitor => {
                logger.info(
                    "startup",
                    "D3D11 unavailable on this platform; continuing in degraded mode",
                )?;
                None
            }
            Err(error) => return Err(error.into()),
        };

        let loaded_monitors = load_monitors(options.clone())?;
        let desktop_host = match loaded_monitors.native_monitors.as_ref() {
            Some(monitors) => match attach_wallpaper_hosts(monitors) {
                Ok(host) => {
                    logger.info(
                        "startup",
                        format!("attached wallpaper hosts for {} monitors", monitors.len()),
                    )?;
                    Some(host)
                }
                Err(error) if should_fallback_to_synthetic_monitor(&options, &error) => {
                    logger.info(
                        "startup",
                        format!("desktop host unavailable; continuing in degraded mode: {error}"),
                    )?;
                    None
                }
                Err(error) => return Err(error.into()),
            },
            None => {
                logger.info("startup", "using synthetic monitor mode")?;
                None
            }
        };

        let ipc_server = ServiceIpcServer::bind(options.pipe_path)?;
        logger.info(
            "startup",
            format!("named pipe endpoint ready at {}", ipc_server.pipe_path()),
        )?;

        let coordinator = RuntimeCoordinator::new(loaded_monitors.statuses, library);
        logger.info("startup", "runtime coordinator initialized")?;

        Ok(Self {
            coordinator,
            runtime: ServiceRuntime {
                _desktop_host: desktop_host,
                _render_device: render_device,
                _media_runtime: media_runtime,
                ipc_server,
                logger,
            },
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> StatusSnapshot {
        self.coordinator.snapshot()
    }

    #[cfg(test)]
    #[must_use]
    pub fn ipc_pipe_path(&self) -> &str {
        self.runtime.ipc_server.pipe_path()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn handle_command(
        &mut self,
        command: Command,
    ) -> Result<StatusSnapshot, ServiceBootstrapError> {
        self.coordinator.apply_command(command)?;
        Ok(self.coordinator.snapshot())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn handle_envelope(&mut self, envelope: CommandEnvelope) -> ResponseEnvelope {
        if !is_protocol_compatible(envelope.protocol_version) {
            return ResponseEnvelope::error(
                envelope.request_id,
                ControlError::protocol_version_mismatch(envelope.protocol_version),
            );
        }

        match self.handle_command(envelope.command) {
            Ok(snapshot) => ResponseEnvelope::ok(envelope.request_id, Some(snapshot)),
            Err(ServiceBootstrapError::Control { code, message }) => {
                ResponseEnvelope::error(envelope.request_id, ControlError::new(code, message))
            }
            Err(error) => ResponseEnvelope::error(
                envelope.request_id,
                ControlError::new(ControlErrorCode::ServiceUnavailable, error.to_string()),
            ),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn handle_json_request(
        &mut self,
        request_json: &str,
    ) -> Result<String, ServiceBootstrapError> {
        let envelope: CommandEnvelope = serde_json::from_str(request_json)?;
        let response = self.handle_envelope(envelope);
        Ok(serde_json::to_string(&response)?)
    }
}

impl Drop for LiveWallService {
    fn drop(&mut self) {
        let _ = self.runtime.logger.info(
            "shutdown",
            format!(
                "service shutdown complete; pipe endpoint was {}",
                self.runtime.ipc_server.pipe_path()
            ),
        );
    }
}

pub fn run_desktop_smoke_test() -> Result<(), ServiceBootstrapError> {
    attach_smoke_test(Duration::from_secs(3))?;
    Ok(())
}

#[derive(Debug)]
struct LoadedMonitors {
    statuses: Vec<MonitorStatus>,
    native_monitors: Option<Vec<MonitorInfo>>,
}

fn load_monitors(options: ServiceOptions) -> Result<LoadedMonitors, ServiceBootstrapError> {
    load_monitors_with(options, enumerate_monitors)
}

fn load_monitors_with(
    options: ServiceOptions,
    enumerate: fn() -> Result<Vec<MonitorInfo>, DesktopError>,
) -> Result<LoadedMonitors, ServiceBootstrapError> {
    match enumerate() {
        Ok(monitors) => {
            let statuses = monitors
                .iter()
                .map(|monitor| MonitorStatus {
                    monitor_id: monitor.id.clone(),
                    display_name: monitor.display_name.clone(),
                    is_primary: monitor.is_primary,
                    wallpaper_id: None,
                    wallpaper_kind: None,
                    playback_state: PlaybackState::Running,
                })
                .collect();

            Ok(LoadedMonitors {
                statuses,
                native_monitors: Some(monitors),
            })
        }
        Err(error) if should_fallback_to_synthetic_monitor(&options, &error) =>
        {
            Ok(LoadedMonitors {
                statuses: vec![synthetic_monitor()],
                native_monitors: None,
            })
        }
        Err(error) => Err(ServiceBootstrapError::Desktop(error)),
    }
}

fn should_fallback_to_synthetic_monitor(options: &ServiceOptions, error: &DesktopError) -> bool {
    options.allow_synthetic_monitor
        && matches!(
            error,
            DesktopError::UnsupportedPlatform
                | DesktopError::NoMonitors
                | DesktopError::Platform { .. }
        )
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServiceIpcServer {
    pipe_path: String,
    #[cfg(windows)]
    pipe_handle: HANDLE,
}

impl ServiceIpcServer {
    fn bind(pipe_path: impl Into<String>) -> Result<Self, ServiceBootstrapError> {
        let pipe_path = pipe_path.into();
        if pipe_path.trim().is_empty() {
            return Err(ServiceBootstrapError::IpcConfig(
                "pipe path must not be empty".into(),
            ));
        }

        #[cfg(windows)]
        {
            let mut pipe_name_wide: Vec<u16> = pipe_path.encode_utf16().collect();
            if pipe_name_wide.last().copied() != Some(0) {
                pipe_name_wide.push(0);
            }

            let pipe_handle = unsafe {
                CreateNamedPipeW(
                    PCWSTR(pipe_name_wide.as_ptr()),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    64 * 1024,
                    64 * 1024,
                    0,
                    None,
                )
            };
            if pipe_handle == INVALID_HANDLE_VALUE {
                return Err(ServiceBootstrapError::Platform {
                    context: "CreateNamedPipeW",
                    message: std::io::Error::last_os_error().to_string(),
                });
            }

            return Ok(Self {
                pipe_path,
                pipe_handle,
            });
        }

        #[cfg(not(windows))]
        {
            Ok(Self { pipe_path })
        }
    }

    fn pipe_path(&self) -> &str {
        &self.pipe_path
    }
}

impl Drop for ServiceIpcServer {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = CloseHandle(self.pipe_handle);
        }
    }
}

#[derive(Debug)]
struct ServiceLogger {
    file: File,
}

impl ServiceLogger {
    fn open() -> Result<Self, ServiceBootstrapError> {
        let log_dir = service_log_dir();
        create_dir_all(&log_dir)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("service.log"))?;
        Ok(Self { file })
    }

    fn info(
        &mut self,
        step: &'static str,
        message: impl Into<String>,
    ) -> Result<(), ServiceBootstrapError> {
        self.write("info", step, message.into())
    }

    fn write(
        &mut self,
        level: &'static str,
        step: &'static str,
        message: String,
    ) -> Result<(), ServiceBootstrapError> {
        let timestamp_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = serde_json::json!({
            "timestamp_unix": timestamp_unix,
            "level": level,
            "component": "livewall-service",
            "step": step,
            "message": message
        });
        writeln!(self.file, "{entry}")?;
        self.file.flush()?;
        Ok(())
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug)]
struct MediaRuntime {
    com_initialized: bool,
    mf_started: bool,
}

impl MediaRuntime {
    #[cfg(windows)]
    fn initialize() -> Result<Self, ServiceBootstrapError> {
        let mut com_initialized = false;
        let hresult = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hresult.is_ok() {
            com_initialized = true;
        } else if hresult != RPC_E_CHANGED_MODE {
            return Err(ServiceBootstrapError::Platform {
                context: "CoInitializeEx",
                message: hresult.message().to_string(),
            });
        }

        unsafe {
            MFStartup(MF_VERSION, 0).map_err(|error| ServiceBootstrapError::Platform {
                context: "MFStartup",
                message: error.to_string(),
            })?
        };

        Ok(Self {
            com_initialized,
            mf_started: true,
        })
    }

    #[cfg(not(windows))]
    fn initialize() -> Result<Self, ServiceBootstrapError> {
        Err(ServiceBootstrapError::Platform {
            context: "MediaRuntime::initialize",
            message: "unsupported platform".into(),
        })
    }
}

impl Drop for MediaRuntime {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            if self.mf_started {
                let _ = MFShutdown();
            }
            if self.com_initialized {
                CoUninitialize();
            }
        }
    }
}

impl ServiceBootstrapError {
    fn from_control_error(error: ControlError) -> Self {
        Self::Control {
            code: error.code,
            message: error.message,
        }
    }

    fn is_unsupported_platform(&self) -> bool {
        matches!(
            self,
            Self::Platform {
                context: "MediaRuntime::initialize",
                ..
            }
        )
    }
}

impl From<ControlError> for ServiceBootstrapError {
    fn from(value: ControlError) -> Self {
        Self::from_control_error(value)
    }
}

fn service_log_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("LiveWall"));
    base.join("LiveWall").join("logs")
}

#[cfg(test)]
mod tests {
    use super::{
        LiveWallService, ServiceOptions, load_monitors_with, should_fallback_to_synthetic_monitor,
    };
    use livewall_control::{
        Command, CommandEnvelope, ControlErrorCode, PROTOCOL_VERSION, PlaybackState, Response,
    };
    use livewall_desktop::{DesktopError, MonitorInfo};

    #[test]
    fn bootstrap_initializes_with_available_monitors() {
        let service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())
            .expect("service should bootstrap");

        let snapshot = service.snapshot();
        assert!(!snapshot.monitors.is_empty());
        assert_eq!(service.ipc_pipe_path(), r"\\.\pipe\livewall-service");
    }

    #[test]
    fn load_monitors_uses_synthetic_monitor_when_desktop_is_unavailable() {
        fn unsupported_desktop() -> Result<Vec<MonitorInfo>, DesktopError> {
            Err(DesktopError::UnsupportedPlatform)
        }

        let loaded = load_monitors_with(ServiceOptions::default(), unsupported_desktop)
            .expect("service should synthesize a monitor");

        assert_eq!(loaded.statuses.len(), 1);
        assert_eq!(loaded.statuses[0].monitor_id, "SIMULATED_DISPLAY1");
        assert!(loaded.native_monitors.is_none());
    }

    #[test]
    fn synthetic_fallback_supports_platform_errors() {
        let error = DesktopError::Platform {
            context: "EnumWindows(WorkerW)",
            message: "WorkerW host window not found".into(),
        };

        assert!(should_fallback_to_synthetic_monitor(
            &ServiceOptions::default(),
            &error
        ));

        let options = ServiceOptions {
            allow_synthetic_monitor: false,
            ..ServiceOptions::default()
        };
        assert!(!should_fallback_to_synthetic_monitor(&options, &error));
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

    #[test]
    fn get_status_envelope_returns_snapshot() {
        let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())
            .expect("service should bootstrap");
        let response = service.handle_envelope(CommandEnvelope::new(7, Command::GetStatus));

        match response.response {
            Response::Ok { status } => {
                let status = status.expect("status should be present");
                assert_eq!(status.protocol_version, PROTOCOL_VERSION);
                assert!(!status.monitors.is_empty());
            }
            Response::Error { error } => panic!("unexpected error: {error:?}"),
        }
    }

    #[test]
    fn protocol_mismatch_returns_typed_error() {
        let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())
            .expect("service should bootstrap");
        let response = service.handle_envelope(CommandEnvelope {
            protocol_version: PROTOCOL_VERSION + 1,
            request_id: 42,
            command: Command::GetStatus,
        });

        match response.response {
            Response::Error { error } => {
                assert_eq!(error.code, ControlErrorCode::ProtocolVersionMismatch);
                assert_eq!(error.expected_protocol_version, Some(PROTOCOL_VERSION));
                assert_eq!(error.actual_protocol_version, Some(PROTOCOL_VERSION + 1));
            }
            Response::Ok { .. } => panic!("expected protocol mismatch error"),
        }
    }

    #[test]
    fn json_request_round_trip_returns_status() {
        let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())
            .expect("service should bootstrap");
        let request = serde_json::to_string(&CommandEnvelope::new(9, Command::GetStatus))
            .expect("request should serialize");
        let response_json = service
            .handle_json_request(&request)
            .expect("request should be handled");
        let response: livewall_control::ResponseEnvelope =
            serde_json::from_str(&response_json).expect("response should deserialize");

        match response.response {
            Response::Ok { status } => assert!(status.is_some()),
            Response::Error { error } => panic!("unexpected error: {error:?}"),
        }
    }
}
