mod bootstrap;

use std::fs;
use std::path::{Path, PathBuf};

use bootstrap::{LiveWallService, ServiceOptions, run_desktop_smoke_test};
use livewall_control::{Command, CommandEnvelope};
use livewall_pack::{MANIFEST_FILE_NAME, parse_manifest};
use livewall_render::load_scene_descriptor;
use livewall_video::load_video_descriptor;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--serve") => {
            let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())?;
            service.serve()?;
        }
        Some("--serve-once") => {
            let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())?;
            service.serve_once()?;
        }
        Some("--desktop-smoke-test") => {
            run_desktop_smoke_test()?;
        }
        Some("--scene-smoke-test") => {
            let manifest_dir = sample_package_dir("aurora-scene");
            let manifest = load_manifest(&manifest_dir)?;
            let descriptor = load_scene_descriptor(&manifest, &manifest_dir)?;
            println!(
                "scene descriptor loaded: id={}, vertex={}, pixel={}",
                descriptor.wallpaper_id,
                descriptor.vertex_shader_path.display(),
                descriptor.pixel_shader_path.display()
            );
        }
        Some("--video-smoke-test") => {
            let manifest_dir = sample_package_dir("coast-video");
            let manifest = load_manifest(&manifest_dir)?;
            let descriptor = load_video_descriptor(&manifest, &manifest_dir)?;
            println!(
                "video descriptor loaded: id={}, entry={}",
                descriptor.wallpaper_id,
                descriptor.video_path.display()
            );
        }
        Some("--ipc-smoke-test") => {
            let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())?;
            let response = service.handle_envelope(CommandEnvelope::new(1, Command::GetStatus));
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Some("--once") | None => {
            let service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())?;
            println!("{}", serde_json::to_string_pretty(&service.snapshot())?);
        }
        Some(other) => {
            return Err(format!("unsupported argument `{other}`").into());
        }
    }

    Ok(())
}

fn sample_package_dir(id: &str) -> PathBuf {
    Path::new("wallpapers").join("samples").join(id)
}

fn load_manifest(
    package_dir: &Path,
) -> Result<livewall_pack::WallpaperManifest, Box<dyn std::error::Error>> {
    let manifest_path = package_dir.join(MANIFEST_FILE_NAME);
    let source = fs::read_to_string(&manifest_path)?;
    let manifest = parse_manifest(&source)?;
    manifest.validate_assets_exist(package_dir)?;
    Ok(manifest)
}
