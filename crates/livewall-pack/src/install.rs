use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use zip::ZipArchive;

use crate::manifest::{MANIFEST_FILE_NAME, ManifestError, WallpaperManifest, parse_manifest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallLayout {
    pub library_root: PathBuf,
}

impl InstallLayout {
    #[must_use]
    pub fn new(library_root: impl Into<PathBuf>) -> Self {
        Self {
            library_root: library_root.into(),
        }
    }

    #[must_use]
    pub fn wallpaper_dir(&self, wallpaper_id: &str) -> PathBuf {
        self.library_root.join(wallpaper_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledWallpaper {
    pub manifest: WallpaperManifest,
    pub install_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("LOCALAPPDATA is not set")]
    LocalAppDataUnavailable,
    #[error("package is missing `{MANIFEST_FILE_NAME}`")]
    MissingManifest,
    #[error("wallpaper `{id}` is already installed")]
    DuplicateWallpaperId { id: String },
    #[error("archive entry `{path}` escapes the package root")]
    InvalidArchivePath { path: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

pub fn default_library_root() -> Result<PathBuf, InstallError> {
    let local_app_data =
        env::var_os("LOCALAPPDATA").ok_or(InstallError::LocalAppDataUnavailable)?;
    Ok(PathBuf::from(local_app_data)
        .join("LiveWall")
        .join("wallpapers"))
}

pub fn install_wallpaper_package(
    archive_path: &Path,
    layout: &InstallLayout,
) -> Result<InstalledWallpaper, InstallError> {
    fs::create_dir_all(&layout.library_root)?;

    let archive_file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(archive_file)?;

    let manifest = read_manifest(&mut archive)?;
    let install_dir = layout.wallpaper_dir(&manifest.id);
    if install_dir.exists() {
        return Err(InstallError::DuplicateWallpaperId {
            id: manifest.id.clone(),
        });
    }

    let staging_dir = new_staging_dir(layout, &manifest);
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir)?;

    if let Err(error) = extract_archive(&mut archive, &staging_dir)
        .and_then(|()| {
            manifest
                .validate_assets_exist(&staging_dir)
                .map_err(InstallError::from)
        })
        .and_then(|()| fs::rename(&staging_dir, &install_dir).map_err(InstallError::from))
    {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }

    Ok(InstalledWallpaper {
        manifest,
        install_dir,
    })
}

fn read_manifest<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<WallpaperManifest, InstallError> {
    let mut file = archive
        .by_name(MANIFEST_FILE_NAME)
        .map_err(|_| InstallError::MissingManifest)?;

    let mut json = String::new();
    file.read_to_string(&mut json)?;
    Ok(parse_manifest(&json)?)
}

fn extract_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    destination: &Path,
) -> Result<(), InstallError> {
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(enclosed_name) = entry.enclosed_name() else {
            return Err(InstallError::InvalidArchivePath {
                path: entry.name().to_string(),
            });
        };
        let output_path = destination.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&output_path)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut output = File::create(&output_path)?;
        std::io::copy(&mut entry, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

fn new_staging_dir(layout: &InstallLayout, manifest: &WallpaperManifest) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    layout
        .library_root
        .join(format!(".tmp-{}-{stamp}", manifest.id))
}
