use std::time::Duration;

use crate::{DesktopError, MonitorInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperHostWindow {
    pub monitor_id: String,
    pub hwnd: isize,
}

#[derive(Debug, Default)]
pub struct DesktopHost {
    pub workerw: isize,
    pub windows: Vec<WallpaperHostWindow>,
}

#[cfg(windows)]
pub fn attach_wallpaper_hosts(monitors: &[MonitorInfo]) -> Result<DesktopHost, DesktopError> {
    platform::attach_wallpaper_hosts(monitors)
}

#[cfg(not(windows))]
pub fn attach_wallpaper_hosts(_: &[MonitorInfo]) -> Result<DesktopHost, DesktopError> {
    Err(DesktopError::UnsupportedPlatform)
}

#[cfg(windows)]
pub fn attach_smoke_test(duration: Duration) -> Result<(), DesktopError> {
    platform::attach_smoke_test(duration)
}

#[cfg(not(windows))]
pub fn attach_smoke_test(_: Duration) -> Result<(), DesktopError> {
    Err(DesktopError::UnsupportedPlatform)
}

#[cfg(windows)]
mod platform {
    use std::time::Duration;

    use windows::Win32::Foundation::{
        COLORREF, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, PAINTSTRUCT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
        EnumWindows, FindWindowExW, FindWindowW, IDC_ARROW, LoadCursorW, MSG, PM_REMOVE,
        PeekMessageW, PostQuitMessage, RegisterClassW, SMTO_NORMAL, SW_SHOW, SWP_NOACTIVATE,
        SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos, ShowWindow, TranslateMessage,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WM_PAINT, WNDCLASSW, WS_CHILD, WS_VISIBLE,
    };
    use windows::core::{BOOL, w};

    use super::{DesktopHost, WallpaperHostWindow};
    use crate::{DesktopError, MonitorInfo};

    const LIVEWALL_HOST_CLASS: windows::core::PCWSTR = w!("LiveWallHostWindow");
    const PROGMAN_SPAWN_WORKERW: u32 = 0x052C;

    pub fn attach_wallpaper_hosts(monitors: &[MonitorInfo]) -> Result<DesktopHost, DesktopError> {
        if monitors.is_empty() {
            return Err(DesktopError::NoMonitors);
        }

        ensure_window_class()?;
        let workerw = find_workerw()?;
        let mut windows = Vec::with_capacity(monitors.len());

        for monitor in monitors {
            let hwnd = create_host_window(workerw, monitor)?;
            windows.push(WallpaperHostWindow {
                monitor_id: monitor.id.clone(),
                hwnd: hwnd.0 as isize,
            });
        }

        Ok(DesktopHost {
            workerw: workerw.0 as isize,
            windows,
        })
    }

    pub fn attach_smoke_test(duration: Duration) -> Result<(), DesktopError> {
        let monitors = crate::enumerate_monitors()?;
        let host = attach_wallpaper_hosts(&monitors)?;

        for window in &host.windows {
            unsafe {
                let hwnd = HWND(window.hwnd as *mut core::ffi::c_void);
                let _ = InvalidateRect(Some(hwnd), None, true);
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
        }

        pump_messages(duration);
        drop(host);
        Ok(())
    }

    impl Drop for DesktopHost {
        fn drop(&mut self) {
            for window in &self.windows {
                unsafe {
                    let hwnd = HWND(window.hwnd as *mut core::ffi::c_void);
                    if !hwnd.is_invalid() {
                        let _ = DestroyWindow(hwnd);
                    }
                }
            }
        }
    }

    fn ensure_window_class() -> Result<(), DesktopError> {
        let instance = unsafe {
            GetModuleHandleW(None).map_err(|error| DesktopError::Platform {
                context: "GetModuleHandleW",
                message: error.to_string(),
            })?
        };

        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            hInstance: instance.into(),
            lpszClassName: LIVEWALL_HOST_CLASS,
            lpfnWndProc: Some(host_window_proc),
            ..Default::default()
        };

        let atom = unsafe { RegisterClassW(&class) };
        if atom == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_CLASS_ALREADY_EXISTS.0 as i32) {
                return Err(DesktopError::Platform {
                    context: "RegisterClassW",
                    message: error.to_string(),
                });
            }
        }

        Ok(())
    }

    fn find_workerw() -> Result<HWND, DesktopError> {
        let progman = unsafe {
            FindWindowW(w!("Progman"), None).map_err(|error| DesktopError::Platform {
                context: "FindWindowW(Progman)",
                message: error.to_string(),
            })?
        };

        unsafe {
            let _ = SendMessageTimeoutW(
                progman,
                PROGMAN_SPAWN_WORKERW,
                WPARAM(0),
                LPARAM(0),
                SMTO_NORMAL,
                1000,
                None,
            );
        }

        struct SearchState {
            workerw: Option<HWND>,
        }

        unsafe extern "system" fn enum_windows_proc(window: HWND, lparam: LPARAM) -> BOOL {
            let state = unsafe { &mut *(lparam.0 as *mut SearchState) };
            let shell_view =
                unsafe { FindWindowExW(Some(window), None, w!("SHELLDLL_DefView"), None).ok() };
            if shell_view.is_some() {
                let workerw =
                    unsafe { FindWindowExW(None, Some(window), w!("WorkerW"), None).ok() };
                if let Some(workerw) = workerw {
                    state.workerw = Some(workerw);
                    return false.into();
                }
            }
            true.into()
        }

        let mut state = SearchState { workerw: None };
        unsafe {
            EnumWindows(
                Some(enum_windows_proc),
                LPARAM((&mut state as *mut SearchState).cast::<()>() as isize),
            )
            .map_err(|error| DesktopError::Platform {
                context: "EnumWindows(WorkerW)",
                message: error.to_string(),
            })?;
        }

        state.workerw.ok_or_else(|| DesktopError::Platform {
            context: "EnumWindows(WorkerW)",
            message: "WorkerW host window not found".to_string(),
        })
    }

    fn create_host_window(workerw: HWND, monitor: &MonitorInfo) -> Result<HWND, DesktopError> {
        let instance = unsafe {
            GetModuleHandleW(None).map_err(|error| DesktopError::Platform {
                context: "GetModuleHandleW",
                message: error.to_string(),
            })?
        };

        let bounds = monitor.bounds_px;
        let width = bounds.width().max(1);
        let height = bounds.height().max(1);

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                LIVEWALL_HOST_CLASS,
                w!(""),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
                bounds.left,
                bounds.top,
                width,
                height,
                Some(workerw),
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|error| DesktopError::Platform {
                context: "CreateWindowExW",
                message: error.to_string(),
            })?
        };

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetWindowPos(
                hwnd,
                None,
                bounds.left,
                bounds.top,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }

        Ok(hwnd)
    }

    fn pump_messages(duration: Duration) {
        let deadline = std::time::Instant::now() + duration;
        while std::time::Instant::now() < deadline {
            unsafe {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    unsafe extern "system" fn host_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
                let brush = unsafe { CreateSolidBrush(COLORREF(0)) };
                unsafe {
                    let _ = FillRect(hdc, &paint.rcPaint, brush);
                    let _ = DeleteObject(brush.into());
                    let _ = EndPaint(hwnd, &paint);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }
}
