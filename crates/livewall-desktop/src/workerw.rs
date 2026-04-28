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
    use std::sync::OnceLock;

    use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BLACK_BRUSH, BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
        GetStockObject, HBRUSH, PAINTSTRUCT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
        EnumWindows, FindWindowExW, FindWindowW, GWLP_HINSTANCE, GetWindowLongPtrW, HMENU,
        IDC_ARROW, InvalidateRect, LoadCursorW, MSG, PeekMessageW, PostQuitMessage, RegisterClassW,
        SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos, ShowWindow,
        TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WM_PAINT, WNDCLASSW, WS_CHILD,
        WS_VISIBLE,
    };
    use windows::core::w;

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
                hwnd: hwnd.0,
            });
        }

        Ok(DesktopHost {
            workerw: workerw.0,
            windows,
        })
    }

    pub fn attach_smoke_test(duration: Duration) -> Result<(), DesktopError> {
        let monitors = crate::enumerate_monitors()?;
        let host = attach_wallpaper_hosts(&monitors)?;

        for window in &host.windows {
            unsafe {
                let hwnd = HWND(window.hwnd);
                InvalidateRect(hwnd, None, true);
                ShowWindow(hwnd, SW_SHOW);
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
                    let hwnd = HWND(window.hwnd);
                    if hwnd.0 != 0 {
                        let _ = DestroyWindow(hwnd);
                    }
                }
            }
        }
    }

    fn ensure_window_class() -> Result<(), DesktopError> {
        static WINDOW_CLASS: OnceLock<()> = OnceLock::new();
        WINDOW_CLASS.get_or_try_init(|| {
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
                return Err(DesktopError::Platform {
                    context: "RegisterClassW",
                    message: std::io::Error::last_os_error().to_string(),
                });
            }

            Ok(())
        })?;
        Ok(())
    }

    fn find_workerw() -> Result<HWND, DesktopError> {
        let progman = unsafe { FindWindowW(w!("Progman"), None) };
        if progman.0 == 0 {
            return Err(DesktopError::Platform {
                context: "FindWindowW(Progman)",
                message: "desktop host window not found".to_string(),
            });
        }

        unsafe {
            let _ = SendMessageTimeoutW(
                progman,
                PROGMAN_SPAWN_WORKERW,
                WPARAM(0),
                LPARAM(0),
                Default::default(),
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
                unsafe { FindWindowExW(window, HWND(0), w!("SHELLDLL_DefView"), None) };
            if shell_view.0 != 0 {
                let workerw = unsafe { FindWindowExW(HWND(0), window, w!("WorkerW"), None) };
                if workerw.0 != 0 {
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
            );
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
                workerw,
                HMENU(0),
                instance,
                None,
            )
        };

        if hwnd.0 == 0 {
            return Err(DesktopError::Platform {
                context: "CreateWindowExW",
                message: std::io::Error::last_os_error().to_string(),
            });
        }

        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            let _ = SetWindowPos(
                hwnd,
                HWND(0),
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
                while PeekMessageW(&mut message, HWND(0), 0, 0, 1).as_bool() {
                    TranslateMessage(&message);
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
                    let _ = DeleteObject(brush);
                    EndPaint(hwnd, &paint);
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
