use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MANIFEST_FILE_NAME: &str = "manifest.json";
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperManifest {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub preview: PathBuf,
    #[serde(flatten)]
    pub kind: WallpaperKind,
}

impl WallpaperManifest {
    #[must_use]
    pub fn install_dir(&self, layout: &Path) -> PathBuf {
        layout.join(&self.id)
    }

    #[must_use]
    pub fn required_assets(&self) -> Vec<&Path> {
        let mut assets = vec![self.preview.as_path()];
        match &self.kind {
            WallpaperKind::Video { entry, .. } => assets.push(entry.as_path()),
            WallpaperKind::Scene {
                vertex_shader,
                pixel_shader,
                config,
            } => {
                assets.push(vertex_shader.as_path());
                assets.push(pixel_shader.as_path());
                if let Some(config) = config.as_deref() {
                    assets.push(config);
                }
            }
        }
        assets
    }

    pub fn validate_assets_exist(&self, root: &Path) -> Result<(), ManifestError> {
        for asset in self.required_assets() {
            let path = root.join(asset);
            if !path.is_file() {
                return Err(ManifestError::MissingAsset {
                    path: asset.to_path_buf(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WallpaperKind {
    Video {
        entry: PathBuf,
        #[serde(default)]
        loop_mode: LoopMode,
    },
    Scene {
        vertex_shader: PathBuf,
        pixel_shader: PathBuf,
        #[serde(default)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    Once,
    #[default]
    Loop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperSummary {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("manifest JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("manifest version {version} is not supported")]
    UnsupportedVersion { version: u32 },
    #[error("wallpaper id is empty")]
    EmptyId,
    #[error("wallpaper id `{id}` contains unsupported characters")]
    InvalidId { id: String },
    #[error("manifest path `{path}` must be relative and stay inside the package")]
    InvalidPath { path: PathBuf },
    #[error("manifest asset `{path}` is missing")]
    MissingAsset { path: PathBuf },
}

pub fn parse_manifest(source: &str) -> Result<WallpaperManifest, ManifestError> {
    let manifest: WallpaperManifest = serde_json::from_str(source)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &WallpaperManifest) -> Result<(), ManifestError> {
    if manifest.version != SUPPORTED_MANIFEST_VERSION {
        return Err(ManifestError::UnsupportedVersion {
            version: manifest.version,
        });
    }

    if manifest.id.trim().is_empty() {
        return Err(ManifestError::EmptyId);
    }

    if !manifest
        .id
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(ManifestError::InvalidId {
            id: manifest.id.clone(),
        });
    }

    validate_asset_path(&manifest.preview)?;
    match &manifest.kind {
        WallpaperKind::Video { entry, .. } => validate_asset_path(entry)?,
        WallpaperKind::Scene {
            vertex_shader,
            pixel_shader,
            config,
        } => {
            validate_asset_path(vertex_shader)?;
            validate_asset_path(pixel_shader)?;
            if let Some(config) = config {
                validate_asset_path(config)?;
            }
        }
    }

    Ok(())
}

fn validate_asset_path(path: &Path) -> Result<(), ManifestError> {
    if path.as_os_str().is_empty() {
        return Err(ManifestError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    let mut saw_normal = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => saw_normal = true,
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(ManifestError::InvalidPath {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    if !saw_normal {
        return Err(ManifestError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}
