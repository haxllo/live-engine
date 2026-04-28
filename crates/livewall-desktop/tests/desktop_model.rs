use livewall_desktop::{MonitorInfo, RectI32, desktop_extent, normalize_monitors};

fn monitor(
    id: &str,
    is_primary: bool,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> MonitorInfo {
    MonitorInfo {
        id: id.into(),
        display_name: id.into(),
        is_primary,
        bounds_px: RectI32::new(left, top, right, bottom),
        work_area_px: RectI32::new(left, top, right, bottom),
        dpi: 96,
    }
}

#[test]
fn normalize_monitors_orders_primary_then_top_left() {
    let monitors = vec![
        monitor("DISPLAY3", false, 1920, 0, 3840, 1080),
        monitor("DISPLAY1", true, 0, 0, 1920, 1080),
        monitor("DISPLAY2", false, -1280, 120, 0, 1140),
    ];

    let normalized = normalize_monitors(monitors);
    let ids: Vec<_> = normalized
        .iter()
        .map(|monitor| monitor.id.as_str())
        .collect();

    assert_eq!(ids, vec!["DISPLAY1", "DISPLAY2", "DISPLAY3"]);
}

#[test]
fn desktop_extent_covers_all_monitors() {
    let monitors = vec![
        monitor("DISPLAY1", true, 0, 0, 1920, 1080),
        monitor("DISPLAY2", false, -1280, 120, 0, 1140),
        monitor("DISPLAY3", false, 1920, -200, 3200, 1080),
    ];

    let extent = desktop_extent(&monitors).expect("extent should exist");

    assert_eq!(extent, RectI32::new(-1280, -200, 3200, 1140));
}

#[test]
fn rect_width_and_height_follow_bounds() {
    let rect = RectI32::new(-200, 50, 1720, 1130);

    assert_eq!(rect.width(), 1920);
    assert_eq!(rect.height(), 1080);
}
