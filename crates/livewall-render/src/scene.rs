use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use livewall_pack::{WallpaperKind, WallpaperManifest};
use serde::{Deserialize, Serialize};

use crate::{RenderDevice, RenderError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneConfig {
    #[serde(default = "default_clear_color")]
    pub clear_color: [f32; 4],
    #[serde(default = "default_time_scale")]
    pub time_scale: f32,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            clear_color: default_clear_color(),
            time_scale: default_time_scale(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneDescriptor {
    pub wallpaper_id: String,
    pub title: String,
    pub vertex_shader_path: PathBuf,
    pub pixel_shader_path: PathBuf,
    pub config: SceneConfig,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneFrameContext {
    pub elapsed: Duration,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneUniforms {
    pub time_seconds: f32,
    pub resolution: [f32; 2],
    pub clear_color: [f32; 4],
}

#[derive(Debug)]
pub struct ScenePipeline {
    pub descriptor: SceneDescriptor,
    #[cfg(windows)]
    pub(crate) vertex_shader: windows::Win32::Graphics::Direct3D11::ID3D11VertexShader,
    #[cfg(windows)]
    pub(crate) pixel_shader: windows::Win32::Graphics::Direct3D11::ID3D11PixelShader,
}

pub fn load_scene_descriptor(
    manifest: &WallpaperManifest,
    install_dir: &Path,
) -> Result<SceneDescriptor, RenderError> {
    let WallpaperKind::Scene {
        vertex_shader,
        pixel_shader,
        config,
    } = &manifest.kind
    else {
        return Err(RenderError::Scene(format!(
            "wallpaper `{}` is not a scene package",
            manifest.id
        )));
    };

    let vertex_shader_path = install_dir.join(vertex_shader);
    let pixel_shader_path = install_dir.join(pixel_shader);
    if !vertex_shader_path.is_file() {
        return Err(RenderError::Scene(format!(
            "missing vertex shader `{}`",
            vertex_shader.display()
        )));
    }
    if !pixel_shader_path.is_file() {
        return Err(RenderError::Scene(format!(
            "missing pixel shader `{}`",
            pixel_shader.display()
        )));
    }

    let config = match config {
        Some(config_path) => {
            let config_json = fs::read_to_string(install_dir.join(config_path))?;
            serde_json::from_str(&config_json)?
        }
        None => SceneConfig::default(),
    };

    Ok(SceneDescriptor {
        wallpaper_id: manifest.id.clone(),
        title: manifest.title.clone(),
        vertex_shader_path,
        pixel_shader_path,
        config,
    })
}

#[must_use]
pub fn build_scene_uniforms(
    descriptor: &SceneDescriptor,
    frame: SceneFrameContext,
) -> SceneUniforms {
    SceneUniforms {
        time_seconds: frame.elapsed.as_secs_f32() * descriptor.config.time_scale,
        resolution: [frame.width_px as f32, frame.height_px as f32],
        clear_color: descriptor.config.clear_color,
    }
}

#[cfg(windows)]
impl ScenePipeline {
    pub fn from_descriptor(
        device: &RenderDevice,
        descriptor: SceneDescriptor,
    ) -> Result<Self, RenderError> {
        use windows::Win32::Graphics::Direct3D11::{ID3D11PixelShader, ID3D11VertexShader};

        let vertex_shader_bytes = fs::read(&descriptor.vertex_shader_path)?;
        let pixel_shader_bytes = fs::read(&descriptor.pixel_shader_path)?;

        let mut vertex_shader = None::<ID3D11VertexShader>;
        let mut pixel_shader = None::<ID3D11PixelShader>;

        unsafe {
            device
                .device
                .CreateVertexShader(
                    &vertex_shader_bytes,
                    None,
                    Some(&mut vertex_shader as *mut _),
                )
                .map_err(|error| RenderError::Platform {
                    context: "ID3D11Device::CreateVertexShader",
                    message: error.to_string(),
                })?;

            device
                .device
                .CreatePixelShader(&pixel_shader_bytes, None, Some(&mut pixel_shader as *mut _))
                .map_err(|error| RenderError::Platform {
                    context: "ID3D11Device::CreatePixelShader",
                    message: error.to_string(),
                })?;
        }

        Ok(Self {
            descriptor,
            vertex_shader: vertex_shader.ok_or_else(|| RenderError::Platform {
                context: "ID3D11Device::CreateVertexShader",
                message: "vertex shader handle was not returned".to_string(),
            })?,
            pixel_shader: pixel_shader.ok_or_else(|| RenderError::Platform {
                context: "ID3D11Device::CreatePixelShader",
                message: "pixel shader handle was not returned".to_string(),
            })?,
        })
    }
}

#[cfg(not(windows))]
impl ScenePipeline {
    pub fn from_descriptor(_: &RenderDevice, _: SceneDescriptor) -> Result<Self, RenderError> {
        Err(RenderError::UnsupportedPlatform)
    }
}

fn default_clear_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_time_scale() -> f32 {
    1.0
}
