use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderDeviceInfo {
    pub backend: &'static str,
    pub adapter_name: Option<String>,
}

#[derive(Debug)]
pub struct RenderDevice {
    pub info: RenderDeviceInfo,
    #[cfg(windows)]
    pub(crate) device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    #[cfg(windows)]
    pub(crate) context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("renderer is only available on Windows")]
    UnsupportedPlatform,
    #[error("scene loading failed: {0}")]
    Scene(String),
    #[error("{context} failed: {message}")]
    Platform {
        context: &'static str,
        message: String,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(windows)]
pub fn create_shared_device() -> Result<RenderDevice, RenderError> {
    platform::create_shared_device()
}

#[cfg(not(windows))]
pub fn create_shared_device() -> Result<RenderDevice, RenderError> {
    Err(RenderError::UnsupportedPlatform)
}

#[cfg(windows)]
mod platform {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG, D3D11_SDK_VERSION,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
    };

    use super::{RenderDevice, RenderDeviceInfo, RenderError};

    pub fn create_shared_device() -> Result<RenderDevice, RenderError> {
        let mut device = None::<ID3D11Device>;
        let mut context = None::<ID3D11DeviceContext>;
        let feature_levels = &[];
        let mut chosen_feature_level = Default::default();

        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device as *mut _),
                Some(&mut chosen_feature_level as *mut _),
                Some(&mut context as *mut _),
            )
            .or_else(|_| {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_DEBUG,
                    Some(feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device as *mut _),
                    Some(&mut chosen_feature_level as *mut _),
                    Some(&mut context as *mut _),
                )
            })
            .map_err(|error| RenderError::Platform {
                context: "D3D11CreateDevice",
                message: error.to_string(),
            })?;
        }

        Ok(RenderDevice {
            info: RenderDeviceInfo {
                backend: "d3d11",
                adapter_name: None,
            },
            device: device.ok_or_else(|| RenderError::Platform {
                context: "D3D11CreateDevice",
                message: "device handle was not returned".to_string(),
            })?,
            context: context.ok_or_else(|| RenderError::Platform {
                context: "D3D11CreateDevice",
                message: "device context was not returned".to_string(),
            })?,
        })
    }
}
