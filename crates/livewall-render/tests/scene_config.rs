use std::fs;
use std::time::Duration;

use livewall_pack::{WallpaperKind, parse_manifest};
use livewall_render::{SceneFrameContext, build_scene_uniforms, load_scene_descriptor};
use tempfile::tempdir;

#[test]
fn loads_scene_descriptor_from_manifest_and_files() {
    let temp = tempdir().expect("temp dir should exist");
    let install_dir = temp.path();

    fs::create_dir_all(install_dir.join("shaders")).expect("shader dir should exist");
    fs::write(install_dir.join("preview.jpg"), b"preview").expect("preview should exist");
    fs::write(
        install_dir.join("scene.json"),
        br#"{"clear_color":[0.2,0.3,0.4,1.0],"time_scale":2.0}"#,
    )
    .expect("scene config should exist");
    fs::write(install_dir.join("shaders/fullscreen.vs.cso"), b"vs")
        .expect("vertex shader should exist");
    fs::write(install_dir.join("shaders/aurora.ps.cso"), b"ps").expect("pixel shader should exist");

    let manifest = parse_manifest(
        r#"{
          "id":"aurora-scene",
          "version":1,
          "title":"Aurora",
          "kind":"scene",
          "preview":"preview.jpg",
          "vertex_shader":"shaders/fullscreen.vs.cso",
          "pixel_shader":"shaders/aurora.ps.cso",
          "config":"scene.json"
        }"#,
    )
    .expect("manifest should parse");

    let descriptor =
        load_scene_descriptor(&manifest, install_dir).expect("scene descriptor should load");

    assert_eq!(descriptor.wallpaper_id, "aurora-scene");
    assert_eq!(
        descriptor.vertex_shader_path,
        install_dir.join("shaders/fullscreen.vs.cso")
    );
    assert_eq!(
        descriptor.pixel_shader_path,
        install_dir.join("shaders/aurora.ps.cso")
    );
    assert_eq!(descriptor.config.clear_color, [0.2, 0.3, 0.4, 1.0]);
    assert_eq!(descriptor.config.time_scale, 2.0);
}

#[test]
fn uses_default_scene_config_when_optional_file_is_missing() {
    let temp = tempdir().expect("temp dir should exist");
    let install_dir = temp.path();

    fs::create_dir_all(install_dir.join("shaders")).expect("shader dir should exist");
    fs::write(install_dir.join("preview.jpg"), b"preview").expect("preview should exist");
    fs::write(install_dir.join("shaders/fullscreen.vs.cso"), b"vs")
        .expect("vertex shader should exist");
    fs::write(install_dir.join("shaders/aurora.ps.cso"), b"ps").expect("pixel shader should exist");

    let manifest = parse_manifest(
        r#"{
          "id":"aurora-scene",
          "version":1,
          "title":"Aurora",
          "kind":"scene",
          "preview":"preview.jpg",
          "vertex_shader":"shaders/fullscreen.vs.cso",
          "pixel_shader":"shaders/aurora.ps.cso"
        }"#,
    )
    .expect("manifest should parse");

    let descriptor =
        load_scene_descriptor(&manifest, install_dir).expect("scene descriptor should load");

    assert_eq!(descriptor.config.clear_color, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(descriptor.config.time_scale, 1.0);
}

#[test]
fn rejects_video_manifest_for_scene_loader() {
    let temp = tempdir().expect("temp dir should exist");
    let manifest = parse_manifest(
        r#"{
          "id":"coast-video",
          "version":1,
          "title":"Coast",
          "kind":"video",
          "entry":"video.mp4",
          "preview":"preview.jpg"
        }"#,
    )
    .expect("manifest should parse");

    let error = load_scene_descriptor(&manifest, temp.path())
        .expect_err("video package should not load as scene");

    assert!(error.to_string().contains("not a scene package"));
    assert!(matches!(manifest.kind, WallpaperKind::Video { .. }));
}

#[test]
fn builds_uniforms_from_frame_context() {
    let temp = tempdir().expect("temp dir should exist");
    let install_dir = temp.path();

    fs::create_dir_all(install_dir.join("shaders")).expect("shader dir should exist");
    fs::write(install_dir.join("preview.jpg"), b"preview").expect("preview should exist");
    fs::write(
        install_dir.join("scene.json"),
        br#"{"clear_color":[1.0,0.5,0.25,1.0],"time_scale":0.5}"#,
    )
    .expect("scene config should exist");
    fs::write(install_dir.join("shaders/fullscreen.vs.cso"), b"vs")
        .expect("vertex shader should exist");
    fs::write(install_dir.join("shaders/aurora.ps.cso"), b"ps").expect("pixel shader should exist");

    let manifest = parse_manifest(
        r#"{
          "id":"aurora-scene",
          "version":1,
          "title":"Aurora",
          "kind":"scene",
          "preview":"preview.jpg",
          "vertex_shader":"shaders/fullscreen.vs.cso",
          "pixel_shader":"shaders/aurora.ps.cso",
          "config":"scene.json"
        }"#,
    )
    .expect("manifest should parse");

    let descriptor =
        load_scene_descriptor(&manifest, install_dir).expect("scene descriptor should load");
    let uniforms = build_scene_uniforms(
        &descriptor,
        SceneFrameContext {
            elapsed: Duration::from_secs(6),
            width_px: 1920,
            height_px: 1080,
        },
    );

    assert_eq!(uniforms.time_seconds, 3.0);
    assert_eq!(uniforms.resolution, [1920.0, 1080.0]);
    assert_eq!(uniforms.clear_color, [1.0, 0.5, 0.25, 1.0]);
}
