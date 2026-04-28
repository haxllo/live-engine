use livewall_control::{
    Command, CommandEnvelope, ControlError, ControlErrorCode, Event, EventEnvelope, HealthReport,
    MonitorStatus, PROTOCOL_VERSION, PerformanceMode, PlaybackState, Response, ResponseEnvelope,
    ServiceState, StartupStatus, StatusSnapshot, WallpaperKind, WallpaperSummary,
};

fn sample_status() -> StatusSnapshot {
    StatusSnapshot {
        protocol_version: PROTOCOL_VERSION,
        health: HealthReport {
            state: ServiceState::Ready,
            message: Some("runtime online".into()),
            last_error: None,
        },
        playback_state: PlaybackState::Running,
        performance_mode: PerformanceMode::Balanced,
        startup: StartupStatus { enabled: true },
        monitors: vec![MonitorStatus {
            monitor_id: "DISPLAY1".into(),
            display_name: "Primary Display".into(),
            is_primary: true,
            wallpaper_id: Some("coast-video".into()),
            wallpaper_kind: Some(WallpaperKind::Video),
            playback_state: PlaybackState::Running,
        }],
        library: vec![WallpaperSummary {
            wallpaper_id: "coast-video".into(),
            title: "Coast".into(),
            kind: WallpaperKind::Video,
            preview_path: Some("wallpapers/coast-video/preview.jpg".into()),
        }],
    }
}

#[test]
fn command_round_trip() {
    let command = Command::SetWallpaper {
        monitor_id: "DISPLAY1".into(),
        wallpaper_id: "coast-video".into(),
    };

    let json = serde_json::to_string(&command).expect("command should serialize");
    let decoded: Command = serde_json::from_str(&json).expect("command should deserialize");

    assert_eq!(decoded, command);
}

#[test]
fn command_envelope_round_trip() {
    let envelope = CommandEnvelope::new(7, Command::GetStatus);

    let json = serde_json::to_string(&envelope).expect("envelope should serialize");
    let decoded: CommandEnvelope =
        serde_json::from_str(&json).expect("envelope should deserialize");

    assert_eq!(decoded, envelope);
}

#[test]
fn event_round_trip() {
    let envelope = EventEnvelope::new(Event::StatusPublished {
        status: sample_status(),
    });

    let json = serde_json::to_string(&envelope).expect("event should serialize");
    let decoded: EventEnvelope = serde_json::from_str(&json).expect("event should deserialize");

    assert_eq!(decoded, envelope);
}

#[test]
fn status_snapshot_round_trip() {
    let status = sample_status();

    let json = serde_json::to_string(&status).expect("status should serialize");
    let decoded: StatusSnapshot = serde_json::from_str(&json).expect("status should deserialize");

    assert_eq!(decoded, status);
}

#[test]
fn response_round_trip() {
    let envelope = ResponseEnvelope::ok(21, Some(sample_status()));

    let json = serde_json::to_string(&envelope).expect("response should serialize");
    let decoded: ResponseEnvelope =
        serde_json::from_str(&json).expect("response should deserialize");

    assert_eq!(decoded, envelope);
}

#[test]
fn protocol_mismatch_error_is_typed() {
    let error = ControlError::protocol_version_mismatch(PROTOCOL_VERSION + 1);
    let envelope = ResponseEnvelope::error(99, error.clone());

    assert_eq!(error.code, ControlErrorCode::ProtocolVersionMismatch);
    assert_eq!(error.expected_protocol_version, Some(PROTOCOL_VERSION));
    assert_eq!(error.actual_protocol_version, Some(PROTOCOL_VERSION + 1));

    let json = serde_json::to_string(&envelope).expect("error response should serialize");
    let decoded: ResponseEnvelope =
        serde_json::from_str(&json).expect("error response should deserialize");

    assert_eq!(decoded.response, Response::Error { error });
}
