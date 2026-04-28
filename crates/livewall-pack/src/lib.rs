//! Wallpaper package parsing and installation.

pub mod install;
pub mod manifest;

pub use install::{
    InstallError, InstallLayout, InstalledWallpaper, default_library_root,
    install_wallpaper_package,
};
pub use manifest::{
    LoopMode, MANIFEST_FILE_NAME, ManifestError, SUPPORTED_MANIFEST_VERSION, SceneConfig,
    WallpaperKind, WallpaperManifest, WallpaperSummary, parse_manifest, validate_manifest,
};
