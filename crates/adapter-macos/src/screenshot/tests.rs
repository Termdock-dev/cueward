use std::path::Path;

use super::{
    WindowBounds, WindowScope, ensure_screenshot_file_exists, validate_display,
    validate_user_output_path, windows::WindowCatalogEntry, windows::find_capturable_window,
    windows::parse_window_list_payload, windows::select_capturable_windows,
};

#[test]
fn validate_user_output_path_rejects_parent_components() {
    let err = validate_user_output_path("../shot.png").expect_err("should reject");

    assert!(err.contains("parent directory"));
}

#[test]
fn ensure_screenshot_file_exists_reports_missing_file() {
    let err = ensure_screenshot_file_exists(Path::new("/tmp/does-not-exist-cueward.png"))
        .expect_err("should fail");

    assert!(err.contains("was not created"));
}

#[test]
fn validate_display_rejects_out_of_range_values() {
    let err = validate_display(Some(11)).expect_err("should reject");

    assert!(err.contains("between 1 and 10"));
}

#[test]
fn parse_window_list_payload_reads_window_metadata() {
    let payload = r#"[
      {
        "window_id": 12345,
        "app": "Discord",
        "title": "工程室",
        "owner_pid": 987,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": true,
        "bounds": { "x": 120, "y": 80, "width": 1440, "height": 900 }
      }
    ]"#;

    let windows = parse_window_list_payload(payload).expect("parse");

    assert_eq!(
        windows,
        vec![WindowCatalogEntry {
            window_id: 12345,
            app: "Discord".into(),
            title: "工程室".into(),
            owner_pid: 987,
            layer: 0,
            alpha: 1.0,
            is_onscreen: true,
            is_frontmost: true,
            bounds: WindowBounds {
                x: 120,
                y: 80,
                width: 1440,
                height: 900,
            },
        }]
    );
}

#[test]
fn select_capturable_windows_filters_out_noise_windows() {
    let payload = r#"[
      {
        "window_id": 1,
        "app": "Discord",
        "title": "工程室",
        "owner_pid": 100,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 10, "y": 10, "width": 800, "height": 600 }
      },
      {
        "window_id": 2,
        "app": "Window Server",
        "title": "",
        "owner_pid": 101,
        "layer": 25,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 0, "y": 0, "width": 100, "height": 100 }
      },
      {
        "window_id": 3,
        "app": "Finder",
        "title": "Downloads",
        "owner_pid": 102,
        "layer": 0,
        "alpha": 0.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 0, "y": 0, "width": 900, "height": 700 }
      },
      {
        "window_id": 4,
        "app": "WindowManager",
        "title": "Gesture Blocking Overlay",
        "owner_pid": 103,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 10, "y": 10, "width": 800, "height": 600 }
      }
    ]"#;

    let windows = parse_window_list_payload(payload).expect("parse");
    let selected = select_capturable_windows(windows, WindowScope::OnScreen);

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].window_id, 1);
}

#[test]
fn select_capturable_windows_sorts_frontmost_windows_first() {
    let payload = r#"[
      {
        "window_id": 10,
        "app": "Safari",
        "title": "B",
        "owner_pid": 200,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 0, "y": 0, "width": 800, "height": 600 }
      },
      {
        "window_id": 11,
        "app": "Discord",
        "title": "A",
        "owner_pid": 201,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": true,
        "bounds": { "x": 20, "y": 20, "width": 900, "height": 700 }
      }
    ]"#;

    let windows = parse_window_list_payload(payload).expect("parse");
    let selected = select_capturable_windows(windows, WindowScope::OnScreen);

    assert_eq!(selected[0].window_id, 11);
    assert_eq!(selected[1].window_id, 10);
}

#[test]
fn select_capturable_windows_can_find_window_id() {
    let payload = r#"[
      {
        "window_id": 88,
        "app": "Safari",
        "title": "Docs",
        "owner_pid": 200,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 0, "y": 0, "width": 800, "height": 600 }
      }
    ]"#;

    let windows = select_capturable_windows(
        parse_window_list_payload(payload).expect("parse"),
        WindowScope::OnScreen,
    );

    assert!(windows.iter().any(|window| window.window_id == 88));
}

#[test]
fn find_capturable_window_rejects_missing_id() {
    let payload = r#"[
      {
        "window_id": 88,
        "app": "Safari",
        "title": "Docs",
        "owner_pid": 200,
        "layer": 0,
        "alpha": 1.0,
        "is_onscreen": true,
        "is_frontmost": false,
        "bounds": { "x": 0, "y": 0, "width": 800, "height": 600 }
      }
    ]"#;

    let windows = select_capturable_windows(
        parse_window_list_payload(payload).expect("parse"),
        WindowScope::OnScreen,
    );
    let err = find_capturable_window(&windows, 999).expect_err("missing id should fail");

    assert_eq!(err.to_string(), "window id not found: 999");
}

#[test]
fn all_spaces_includes_offscreen_windows_without_admitting_noise() {
    let bounds = WindowBounds {
        x: -900,
        y: 100,
        width: 800,
        height: 600,
    };
    let visible = WindowCatalogEntry {
        window_id: 1,
        app: "Fixture".into(),
        title: "Visible".into(),
        owner_pid: 100,
        layer: 0,
        alpha: 1.0,
        is_onscreen: true,
        is_frontmost: false,
        bounds,
    };
    let mut hidden = visible.clone();
    hidden.window_id = 2;
    hidden.title = "Other desktop".into();
    hidden.is_onscreen = false;
    let mut overlay = hidden.clone();
    overlay.window_id = 3;
    overlay.layer = 25;
    let mut empty = hidden.clone();
    empty.window_id = 4;
    empty.title.clear();
    let mut transparent = hidden.clone();
    transparent.window_id = 5;
    transparent.alpha = 0.0;
    let entries = vec![visible, hidden, overlay, empty, transparent];
    let default = select_capturable_windows(entries.clone(), WindowScope::OnScreen);
    assert_eq!(
        default.iter().map(|w| w.window_id).collect::<Vec<_>>(),
        vec![1]
    );
    let all = select_capturable_windows(entries, WindowScope::AllSpaces);
    assert_eq!(
        all.iter().map(|w| w.window_id).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert!(!all[0].is_onscreen);
    assert!(all[1].is_onscreen);
    assert_eq!(all[0].bounds.x, -900);
}
