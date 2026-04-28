//! Desktop attachment and monitor discovery.

mod monitors;
mod workerw;

pub use monitors::{MonitorInfo, RectI32, desktop_extent, enumerate_monitors, normalize_monitors};
pub use workerw::{DesktopHost, WallpaperHostWindow, attach_smoke_test, attach_wallpaper_hosts};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("desktop host is only available on Windows")]
    UnsupportedPlatform,
    #[error("no monitors are available")]
    NoMonitors,
    #[error("{context} failed: {message}")]
    Platform {
        context: &'static str,
        message: String,
    },
}
