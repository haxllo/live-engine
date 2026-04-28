use livewall_control::{
    Command, MonitorStatus, PerformanceMode, PlaybackState, WallpaperKind, WallpaperSummary,
};
use livewall_engine::{PolicyState, RuntimeCoordinator, decide_frame_policy};

fn monitor(monitor_id: &str, is_primary: bool) -> MonitorStatus {
    MonitorStatus {
        monitor_id: monitor_id.into(),
        display_name: monitor_id.into(),
        is_primary,
        wallpaper_id: None,
        wallpaper_kind: None,
        playback_state: PlaybackState::Running,
    }
}

fn library() -> Vec<WallpaperSummary> {
    vec![
        WallpaperSummary {
            wallpaper_id: "coast-video".into(),
            title: "Coast".into(),
            kind: WallpaperKind::Video,
            preview_path: Some("wallpapers/coast-video/preview.jpg".into()),
        },
        WallpaperSummary {
            wallpaper_id: "aurora-scene".into(),
            title: "Aurora".into(),
            kind: WallpaperKind::Scene,
            preview_path: Some("wallpapers/aurora-scene/preview.jpg".into()),
        },
    ]
}

#[test]
fn fullscreen_app_forces_pause() {
    let state = PolicyState::default().with_fullscreen_app(true);
    let decision = decide_frame_policy(&state, PerformanceMode::Balanced);

    assert_eq!(decision.playback_state, PlaybackState::Paused);
    assert_eq!(decision.target_fps, 0);
    assert!(!decision.decode_allowed);
}

#[test]
fn battery_saver_caps_frame_rate() {
    let state = PolicyState::default()
        .with_source_fps(60)
        .with_on_battery(true)
        .with_battery_percent(Some(55));

    let decision = decide_frame_policy(&state, PerformanceMode::BatterySaver);

    assert_eq!(decision.playback_state, PlaybackState::Running);
    assert_eq!(decision.target_fps, 24);
    assert!(decision.decode_allowed);
}

#[test]
fn low_battery_battery_saver_pauses_decode() {
    let state = PolicyState::default()
        .with_source_fps(60)
        .with_on_battery(true)
        .with_battery_percent(Some(8));

    let decision = decide_frame_policy(&state, PerformanceMode::BatterySaver);

    assert_eq!(decision.playback_state, PlaybackState::Paused);
    assert_eq!(decision.target_fps, 0);
    assert!(!decision.decode_allowed);
}

#[test]
fn set_wallpaper_updates_monitor_assignment() {
    let mut coordinator = RuntimeCoordinator::new(
        vec![monitor("DISPLAY1", true), monitor("DISPLAY2", false)],
        library(),
    );

    coordinator
        .apply_command(Command::SetWallpaper {
            monitor_id: "DISPLAY2".into(),
            wallpaper_id: "aurora-scene".into(),
        })
        .expect("command should succeed");

    let snapshot = coordinator.snapshot();
    let monitor = snapshot
        .monitors
        .iter()
        .find(|monitor| monitor.monitor_id == "DISPLAY2")
        .expect("monitor should exist");

    assert_eq!(monitor.wallpaper_id.as_deref(), Some("aurora-scene"));
    assert_eq!(monitor.wallpaper_kind, Some(WallpaperKind::Scene));
}

#[test]
fn monitor_hotplug_preserves_existing_assignments() {
    let mut coordinator = RuntimeCoordinator::new(vec![monitor("DISPLAY1", true)], library());
    coordinator
        .apply_command(Command::SetWallpaper {
            monitor_id: "DISPLAY1".into(),
            wallpaper_id: "coast-video".into(),
        })
        .expect("command should succeed");

    coordinator.replace_monitors(vec![monitor("DISPLAY1", true), monitor("DISPLAY3", false)]);

    let snapshot = coordinator.snapshot();
    let display1 = snapshot
        .monitors
        .iter()
        .find(|monitor| monitor.monitor_id == "DISPLAY1")
        .expect("display1 should exist");
    let display3 = snapshot
        .monitors
        .iter()
        .find(|monitor| monitor.monitor_id == "DISPLAY3")
        .expect("display3 should exist");

    assert_eq!(display1.wallpaper_id.as_deref(), Some("coast-video"));
    assert_eq!(display3.wallpaper_id, None);
}
