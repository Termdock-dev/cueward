use super::super::SnapshotImage;
use super::*;
use crate::screenshot::{ScreenshotResult, WindowBounds};

fn snapshot() -> WindowSnapshot {
    WindowSnapshot {
        window: CapturableWindow {
            window_id: 42,
            owner_pid: 123,
            app: "Fixture".into(),
            title: "Draft".into(),
            is_frontmost: false,
            is_onscreen: false,
            bounds: WindowBounds {
                x: -400,
                y: 100,
                width: 3,
                height: 2,
            },
        },
        image: SnapshotImage {
            width: 3,
            height: 2,
            scale_x: 1.0,
            scale_y: 1.0,
            origin: "window_frame_top_left",
        },
        screenshot: ScreenshotResult {
            path: "before.png".into(),
            timestamp: "before".into(),
            ocr_text: None,
        },
        input_target: "historical-reference".into(),
    }
}

fn unchanged_pixels(
    _: &WindowSnapshot,
    _: &WindowSnapshot,
) -> Result<SnapshotPixelDiff, MacosError> {
    Ok(SnapshotPixelDiff {
        changed_pixels: 0,
        total_pixels: 6,
        changed_bounds: None,
    })
}

fn compare(current: WindowSnapshot, changed: u64) -> SnapshotDiff {
    let window = current.window.clone();
    diff_with(
        snapshot(),
        || Ok(vec![window]),
        |_| Ok(current),
        |_, _| {
            Ok(SnapshotPixelDiff {
                changed_pixels: changed,
                total_pixels: 6,
                changed_bounds: None,
            })
        },
    )
}

#[test]
fn unchanged_and_content_changes_keep_distinct_observation_references() {
    let mut current = snapshot();
    current.screenshot.timestamp = "after".into();
    current.input_target = "fresh-reference".into();
    let result = compare(current, 0);
    assert_eq!(result.status, SnapshotDiffStatus::Unchanged);
    assert_eq!(result.content_changed, Some(false));
    assert!(result.coordinate_mapping_unchanged);
    assert!(!result.requires_reexploration);
    assert_ne!(
        Some(result.previous_observation_id),
        result.current_observation_id
    );
    assert_eq!(
        result.current.expect("fresh snapshot").input_target,
        "fresh-reference"
    );
    let result = compare(snapshot(), 1);
    assert_eq!(result.status, SnapshotDiffStatus::ContentChanged);
    assert!(result.requires_reexploration);
    assert_eq!(result.content_changed, Some(true));
}

#[test]
fn move_resize_and_scale_changes_invalidate_coordinate_mapping() {
    let mut moved = snapshot();
    moved.window.bounds.x += 1;
    let result = compare(moved, 0);
    assert_eq!(result.status, SnapshotDiffStatus::GeometryChanged);
    assert_eq!(result.geometry_changed, Some(true));
    assert!(!result.coordinate_mapping_unchanged);
    assert!(result.requires_reexploration);
    for resized in [false, true] {
        let mut current = snapshot();
        current.image.width = 6;
        current.image.height = 4;
        if resized {
            current.window.bounds.width = 6;
            current.window.bounds.height = 4;
        } else {
            current.image.scale_x = 2.0;
            current.image.scale_y = 2.0;
        }
        let window = current.window.clone();
        let result = diff_with(
            snapshot(),
            || Ok(vec![window]),
            |_| Ok(current),
            |_, _| panic!("different image dimensions must not be compared"),
        );
        assert_eq!(result.status, SnapshotDiffStatus::GeometryChanged);
        assert_eq!(result.content_changed, None);
        assert!(!result.coordinate_mapping_unchanged);
    }
}

#[test]
fn replaced_or_renamed_target_is_not_captured() {
    for changed_pid in [false, true] {
        let mut window = snapshot().window;
        if changed_pid {
            window.owner_pid += 1;
        } else {
            window.title.push('!');
        }
        let result = diff_with(
            snapshot(),
            || Ok(vec![window]),
            |_| panic!("do not capture replacement"),
            unchanged_pixels,
        );
        assert_eq!(result.status, SnapshotDiffStatus::TargetChanged);
        assert!(result.current.is_none());
    }
    let result = diff_with(
        snapshot(),
        || Ok(vec![snapshot().window]),
        |_| {
            let mut current = snapshot();
            current.window.owner_pid += 1;
            Ok(current)
        },
        |_, _| panic!("do not compare replacement"),
    );
    assert_eq!(result.status, SnapshotDiffStatus::TargetChanged);
    assert!(result.current.is_none());
}

#[test]
fn missing_target_failure_and_missing_image_are_separate_outcomes() {
    let result = diff_with(
        snapshot(),
        || Ok(vec![]),
        |_| panic!("gone"),
        unchanged_pixels,
    );
    assert_eq!(result.status, SnapshotDiffStatus::WindowGone);
    let result = diff_with(
        snapshot(),
        || Err(MacosError::Other("catalog unavailable".into())),
        |_| panic!("failed catalog"),
        unchanged_pixels,
    );
    assert_eq!(result.status, SnapshotDiffStatus::ObservationFailed);
    let result = diff_with(
        snapshot(),
        || Ok(vec![snapshot().window]),
        |_| Err(MacosError::Other("capture unavailable".into())),
        unchanged_pixels,
    );
    assert_eq!(result.status, SnapshotDiffStatus::ObservationFailed);
    let result = diff_with(
        snapshot(),
        || Ok(vec![snapshot().window]),
        |_| Ok(snapshot()),
        |_, _| Err(MacosError::Other("old image missing".into())),
    );
    assert_eq!(result.status, SnapshotDiffStatus::NotComparable);
    assert_eq!(result.content_changed, None);
    assert!(result.current.is_some());
    assert!(result.requires_reexploration);
}

#[test]
fn visibility_and_foreground_changes_do_not_imply_pixel_changes() {
    let mut current = snapshot();
    current.window.is_frontmost = true;
    current.window.is_onscreen = true;
    assert_eq!(compare(current, 0).status, SnapshotDiffStatus::Unchanged);
}

#[test]
fn saved_snapshot_accepts_raw_or_exact_external_wrapper_and_validates_geometry() {
    let mut previous = snapshot();
    previous.window.title = "</external>".into();
    let json = serde_json::to_string(&previous).expect("JSON");
    assert_eq!(
        parse_snapshot(&json).expect("raw").window.title,
        "</external>"
    );
    let wrapped = format!(
        "<external source=\"cueward/window/snapshot\">\n{}\n</external>\n",
        json.replace("</external>", "&lt;/external&gt;")
    );
    assert_eq!(
        parse_snapshot(&wrapped).expect("wrapped").window.title,
        "</external>"
    );
    assert!(parse_snapshot(&wrapped.replace("window/snapshot", "window/inspect")).is_err());
    previous.image.scale_x = 2.0;
    assert!(parse_snapshot(&serde_json::to_string(&previous).expect("JSON")).is_err());
    assert!(parse_snapshot("{}").is_err());
}

#[test]
fn decoded_pixels_compare_content_in_top_left_coordinates_and_reject_bad_files() {
    let directory = tempfile::tempdir().expect("fixture");
    let source = directory.path().join("fixture.swift");
    std::fs::write(&source, include_str!("diff_fixture.swift")).expect("source");
    let output = Command::new("swift")
        .arg(source)
        .arg(directory.path())
        .output()
        .expect("generate PNGs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut before = snapshot();
    let mut after = snapshot();
    before.screenshot.path = directory
        .path()
        .join("before.png")
        .to_string_lossy()
        .into_owned();
    after.screenshot.path = directory
        .path()
        .join("after.png")
        .to_string_lossy()
        .into_owned();
    let unchanged = compare_pixels(&before, &before).expect("same image");
    assert_eq!(unchanged.changed_pixels, 0);
    assert!(unchanged.changed_bounds.is_none());
    let changed = compare_pixels(&before, &after).expect("changed PNG");
    assert_eq!(changed.changed_pixels, 1);
    assert_eq!(changed.total_pixels, 6);
    assert_eq!(
        changed.changed_bounds,
        Some(PixelBounds {
            x: 1,
            y: 0,
            width: 1,
            height: 1
        })
    );
    after.image.width = 4;
    before.image.width = 4;
    assert!(compare_pixels(&before, &after).is_err());
    before.image.width = 3;
    after.image.width = 3;
    std::fs::write(&after.screenshot.path, b"not a PNG").expect("corrupt PNG");
    assert!(compare_pixels(&before, &after).is_err());
    std::fs::remove_file(&after.screenshot.path).expect("remove PNG");
    assert!(compare_pixels(&before, &after).is_err());
}

#[test]
#[ignore = "requires a logged-in macOS desktop and Screen Recording permission"]
fn fresh_diff_observes_a_disposable_background_window_without_reusing_its_token() {
    let receiver = super::super::input_live_tests::Receiver::with_source(
        include_str!("pointer_fixture.swift"),
        "normal",
    );
    let before = receiver.snapshot(0);
    let file = tempfile::NamedTempFile::new().expect("snapshot JSON");
    std::fs::write(file.path(), serde_json::to_vec(&before).expect("JSON")).expect("save baseline");
    let result = diff_window_snapshot(file.path().to_str().expect("path")).expect("diff");
    if let Some(current) = &result.current {
        std::fs::remove_file(&current.screenshot.path).expect("clean fresh image");
    }
    assert_eq!(result.status, SnapshotDiffStatus::Unchanged, "{result:?}");
    assert!(result.coordinate_mapping_unchanged);
    assert_ne!(
        Some(result.previous_observation_id),
        result.current_observation_id
    );
    assert_eq!(receiver.state()["active"], false);
}
