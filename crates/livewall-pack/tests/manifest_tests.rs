use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use livewall_pack::{
    InstallError, InstallLayout, LoopMode, MANIFEST_FILE_NAME, ManifestError, WallpaperKind,
    install_wallpaper_package, parse_manifest,
};
use tempfile::tempdir;
use zip::ZipWriter;
use zip::write::FileOptions;

fn create_archive(path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("archive should be created");
    let mut archive = ZipWriter::new(file);
    let options = FileOptions::default();

    for (name, contents) in entries {
        archive
            .start_file(*name, options)
            .expect("archive entry should start");
        archive
            .write_all(contents)
            .expect("archive entry should be written");
    }

    archive.finish().expect("archive should finish");
}

#[test]
fn parses_video_manifest() {
    let manifest = r#"{
      "id":"coast-video",
      "version":1,
      "title":"Coast",
      "kind":"video",
      "entry":"video.mp4",
      "preview":"preview.jpg"
    }"#;

    let parsed = parse_manifest(manifest).expect("video manifest should parse");

    assert_eq!(parsed.id, "coast-video");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.title, "Coast");
    assert_eq!(parsed.preview, Path::new("preview.jpg"));
    assert_eq!(
        parsed.kind,
        WallpaperKind::Video {
            entry: "video.mp4".into(),
            loop_mode: LoopMode::Loop,
        }
    );
}

#[test]
fn parses_scene_manifest() {
    let manifest = r#"{
      "id":"aurora-scene",
      "version":1,
      "title":"Aurora",
      "kind":"scene",
      "preview":"preview.jpg",
      "vertex_shader":"shaders/fullscreen.vs.cso",
      "pixel_shader":"shaders/aurora.ps.cso",
      "config":"scene.json"
    }"#;

    let parsed = parse_manifest(manifest).expect("scene manifest should parse");

    assert_eq!(parsed.id, "aurora-scene");
    assert_eq!(
        parsed.kind,
        WallpaperKind::Scene {
            vertex_shader: "shaders/fullscreen.vs.cso".into(),
            pixel_shader: "shaders/aurora.ps.cso".into(),
            config: Some("scene.json".into()),
        }
    );
}

#[test]
fn rejects_unsupported_manifest_version() {
    let manifest = r#"{
      "id":"coast-video",
      "version":2,
      "title":"Coast",
      "kind":"video",
      "entry":"video.mp4",
      "preview":"preview.jpg"
    }"#;

    let error = parse_manifest(manifest).expect_err("unsupported version should fail");
    assert!(matches!(
        error,
        ManifestError::UnsupportedVersion { version: 2 }
    ));
}

#[test]
fn rejects_asset_path_escape() {
    let manifest = r#"{
      "id":"coast-video",
      "version":1,
      "title":"Coast",
      "kind":"video",
      "entry":"../video.mp4",
      "preview":"preview.jpg"
    }"#;

    let error = parse_manifest(manifest).expect_err("path traversal should fail");
    assert!(matches!(error, ManifestError::InvalidPath { .. }));
}

#[test]
fn installs_wallpaper_archive() {
    let temp = tempdir().expect("temp dir should exist");
    let archive_path = temp.path().join("coast-video.livewall");
    create_archive(
        &archive_path,
        &[
            (
                MANIFEST_FILE_NAME,
                br#"{
                    "id":"coast-video",
                    "version":1,
                    "title":"Coast",
                    "kind":"video",
                    "entry":"video.mp4",
                    "preview":"preview.jpg"
                }"#,
            ),
            ("preview.jpg", b"preview-bytes"),
            ("video.mp4", b"video-bytes"),
        ],
    );

    let layout = InstallLayout::new(temp.path().join("library"));
    let installed =
        install_wallpaper_package(&archive_path, &layout).expect("package should install");

    assert_eq!(installed.manifest.id, "coast-video");
    assert_eq!(installed.install_dir, layout.wallpaper_dir("coast-video"));
    assert!(installed.install_dir.join("preview.jpg").is_file());
    assert!(installed.install_dir.join("video.mp4").is_file());
}

#[test]
fn rejects_duplicate_wallpaper_id_on_install() {
    let temp = tempdir().expect("temp dir should exist");
    let archive_path = temp.path().join("coast-video.livewall");
    create_archive(
        &archive_path,
        &[
            (
                MANIFEST_FILE_NAME,
                br#"{
                    "id":"coast-video",
                    "version":1,
                    "title":"Coast",
                    "kind":"video",
                    "entry":"video.mp4",
                    "preview":"preview.jpg"
                }"#,
            ),
            ("preview.jpg", b"preview-bytes"),
            ("video.mp4", b"video-bytes"),
        ],
    );

    let layout = InstallLayout::new(temp.path().join("library"));
    install_wallpaper_package(&archive_path, &layout).expect("first install should succeed");

    let error = install_wallpaper_package(&archive_path, &layout)
        .expect_err("duplicate install should fail");

    assert!(matches!(
        error,
        InstallError::DuplicateWallpaperId { id } if id == "coast-video"
    ));
}

#[test]
fn rejects_archive_missing_manifest_assets() {
    let temp = tempdir().expect("temp dir should exist");
    let archive_path = temp.path().join("aurora.livewall");
    create_archive(
        &archive_path,
        &[
            (
                MANIFEST_FILE_NAME,
                br#"{
                    "id":"aurora-scene",
                    "version":1,
                    "title":"Aurora",
                    "kind":"scene",
                    "preview":"preview.jpg",
                    "vertex_shader":"shaders/fullscreen.vs.cso",
                    "pixel_shader":"shaders/aurora.ps.cso"
                }"#,
            ),
            ("preview.jpg", b"preview-bytes"),
            ("shaders/fullscreen.vs.cso", b"vs-bytes"),
        ],
    );

    let layout = InstallLayout::new(temp.path().join("library"));
    let error =
        install_wallpaper_package(&archive_path, &layout).expect_err("missing assets should fail");

    assert!(matches!(
        error,
        InstallError::Manifest(ManifestError::MissingAsset { .. })
    ));
    assert!(!layout.wallpaper_dir("aurora-scene").exists());
    assert_eq!(
        fs::read_dir(&layout.library_root)
            .expect("library root should exist")
            .count(),
        0
    );
}
