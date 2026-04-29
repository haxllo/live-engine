use crate::DesktopError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RectI32 {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectI32 {
    #[must_use]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[must_use]
    pub const fn width(self) -> i32 {
        self.right - self.left
    }

    #[must_use]
    pub const fn height(self) -> i32 {
        self.bottom - self.top
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    pub id: String,
    pub display_name: String,
    pub is_primary: bool,
    pub bounds_px: RectI32,
    pub work_area_px: RectI32,
    pub dpi: u32,
}

pub fn normalize_monitors(mut monitors: Vec<MonitorInfo>) -> Vec<MonitorInfo> {
    monitors.sort_by(|left, right| {
        (
            !left.is_primary,
            left.bounds_px.left,
            left.bounds_px.top,
            &left.id,
        )
            .cmp(&(
                !right.is_primary,
                right.bounds_px.left,
                right.bounds_px.top,
                &right.id,
            ))
    });
    monitors
}

#[must_use]
pub fn desktop_extent(monitors: &[MonitorInfo]) -> Option<RectI32> {
    let mut iter = monitors.iter();
    let first = iter.next()?.bounds_px;
    Some(iter.fold(first, |bounds, monitor| bounds.union(monitor.bounds_px)))
}

#[cfg(windows)]
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>, DesktopError> {
    platform::enumerate_monitors()
}

#[cfg(not(windows))]
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>, DesktopError> {
    Err(DesktopError::UnsupportedPlatform)
}

#[cfg(windows)]
mod platform {
    use std::mem::size_of;

    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
    use windows::core::BOOL;

    use super::{MonitorInfo, RectI32, normalize_monitors};
    use crate::DesktopError;

    struct EnumState {
        monitors: Vec<MonitorInfo>,
        error: Option<DesktopError>,
    }

    pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>, DesktopError> {
        let mut state = EnumState {
            monitors: Vec::new(),
            error: None,
        };

        let result = unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(enum_monitor_proc),
                LPARAM((&mut state as *mut EnumState).cast::<()>() as isize),
            )
        };

        if !result.as_bool() {
            if let Some(error) = state.error {
                return Err(error);
            }
            return Err(DesktopError::Platform {
                context: "EnumDisplayMonitors",
                message: std::io::Error::last_os_error().to_string(),
            });
        }

        if state.monitors.is_empty() {
            return Err(DesktopError::NoMonitors);
        }

        Ok(normalize_monitors(state.monitors))
    }

    unsafe extern "system" fn enum_monitor_proc(
        hmonitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut EnumState) };
        match read_monitor_info(hmonitor) {
            Ok(monitor) => {
                state.monitors.push(monitor);
                true.into()
            }
            Err(error) => {
                state.error = Some(error);
                false.into()
            }
        }
    }

    fn read_monitor_info(hmonitor: HMONITOR) -> Result<MonitorInfo, DesktopError> {
        let mut monitor_info = MONITORINFOEXW::default();
        monitor_info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

        unsafe {
            GetMonitorInfoW(
                hmonitor,
                (&mut monitor_info.monitorInfo as *mut MONITORINFO).cast(),
            )
            .ok()
            .map_err(|error| DesktopError::Platform {
                context: "GetMonitorInfoW",
                message: error.to_string(),
            })?;
        }

        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let _ = unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };

        let monitor = monitor_info.monitorInfo.rcMonitor;
        let work_area = monitor_info.monitorInfo.rcWork;
        let id = wide_to_string(&monitor_info.szDevice);

        Ok(MonitorInfo {
            id: id.clone(),
            display_name: id,
            is_primary: (monitor_info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
            bounds_px: RectI32::new(monitor.left, monitor.top, monitor.right, monitor.bottom),
            work_area_px: RectI32::new(
                work_area.left,
                work_area.top,
                work_area.right,
                work_area.bottom,
            ),
            dpi: dpi_x.max(dpi_y),
        })
    }

    fn wide_to_string(buffer: &[u16]) -> String {
        let length = buffer
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    }
}
