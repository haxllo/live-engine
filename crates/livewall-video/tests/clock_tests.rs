use std::fs;
use std::time::Duration;

use livewall_pack::{LoopMode, parse_manifest};
use livewall_video::{ClockState, PlaybackClock, load_video_descriptor};
use tempfile::tempdir;

#[test]
fn loop_mode_wraps_when_clock_crosses_duration() {
    let mut clock = PlaybackClock::new(Duration::from_secs(10), LoopMode::Loop, 30.0);

    clock.play(Duration::from_secs(0));
    let snapshot = clock.update(Duration::from_secs(12));

    assert_eq!(snapshot.state, ClockState::Playing);
    assert_eq!(snapshot.position, Duration::from_secs(2));
    assert!(snapshot.wrapped);
}

#[test]
fn pause_and_resume_preserve_position() {
    let mut clock = PlaybackClock::new(Duration::from_secs(20), LoopMode::Loop, 24.0);

    clock.play(Duration::from_secs(0));
    let _ = clock.update(Duration::from_secs(5));
    clock.pause(Duration::from_secs(5));
    let paused = clock.update(Duration::from_secs(9));

    assert_eq!(paused.state, ClockState::Paused);
    assert_eq!(paused.position, Duration::from_secs(5));

    clock.play(Duration::from_secs(9));
    let resumed = clock.update(Duration::from_secs(11));
    assert_eq!(resumed.position, Duration::from_secs(7));
}

#[test]
fn once_mode_ends_at_duration_boundary() {
    let mut clock = PlaybackClock::new(Duration::from_secs(8), LoopMode::Once, 60.0);

    clock.play(Duration::from_secs(0));
    let snapshot = clock.update(Duration::from_secs(9));

    assert_eq!(snapshot.state, ClockState::Ended);
    assert_eq!(snapshot.position, Duration::from_secs(8));
    assert!(!snapshot.wrapped);
    assert!(snapshot.next_frame_deadline.is_none());
}

#[test]
fn next_frame_deadline_tracks_schedule_without_drift() {
    let mut clock = PlaybackClock::new(Duration::from_secs(30), LoopMode::Loop, 25.0);

    clock.play(Duration::ZERO);
    let snapshot = clock.update(Duration::from_millis(100));

    assert_eq!(snapshot.position, Duration::from_millis(100));
    assert_eq!(
        snapshot.next_frame_deadline,
        Some(Duration::from_millis(140))
    );
}

#[test]
fn loads_video_descriptor_from_manifest() {
    let temp = tempdir().expect("temp dir should exist");
    fs::write(temp.path().join("video.mp4"), b"video").expect("video should exist");

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

    let descriptor =
        load_video_descriptor(&manifest, temp.path()).expect("video descriptor should load");

    assert_eq!(descriptor.wallpaper_id, "coast-video");
    assert_eq!(descriptor.video_path, temp.path().join("video.mp4"));
    assert_eq!(descriptor.loop_mode, LoopMode::Loop);
}
