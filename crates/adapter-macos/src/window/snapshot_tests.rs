use std::cell::Cell;
use std::fs;

use super::*;

fn window() -> CapturableWindow {
    CapturableWindow {
        window_id: 42,
        owner_pid: 123,
        app: "Fixture".into(),
        title: "Draft".into(),
        is_frontmost: false,
        is_onscreen: false,
        bounds: WindowBounds {
            x: -800,
            y: 120,
            width: 400,
            height: 300,
        },
    }
}

// The capture stand-in supplies the header consumed by the dimension reader.
fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes
}

fn capture(_ocr: bool, path: &str, _id: u32) -> Result<ScreenshotResult, MacosError> {
    // The system capture writer does not accept hidden destination basenames.
    if Path::new(path)
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
    {
        return Err(MacosError::Other("capture destination rejected".into()));
    }
    fs::write(path, png_header(800, 600)).expect("write capture header");
    Ok(ScreenshotResult {
        path: path.into(),
        timestamp: "capture-time".into(),
        ocr_text: None,
    })
}

#[test]
fn snapshot_can_publish_a_hidden_destination_after_capture() {
    let dir = tempfile::tempdir().expect("directory");
    let output = dir.path().join(".frame.png");
    let result = snapshot_with(42, false, output.to_str(), || Ok(vec![window()]), capture)
        .expect("hidden output is published after capture");
    assert_eq!(result.screenshot.path, output.to_string_lossy());
    assert_eq!(fs::read(&output).expect("image"), png_header(800, 600));
    assert_eq!(fs::read_dir(dir.path()).expect("list").count(), 1);
}

#[test]
fn offscreen_snapshot_publishes_image_with_window_coordinate_metadata() {
    let dir = tempfile::tempdir().expect("directory");
    let output = dir.path().join("result.png");
    fs::write(&output, b"previous image").expect("previous image");
    let result = snapshot_with(
        42,
        true,
        output.to_str(),
        || Ok(vec![window()]),
        |ocr, temporary_path, id| {
            assert!(ocr);
            assert_eq!(id, 42);
            assert_ne!(Path::new(temporary_path), output);
            let mut shot = capture(ocr, temporary_path, id)?;
            shot.ocr_text = Some("Fixture content".into());
            Ok(shot)
        },
    )
    .expect("snapshot");

    assert!(!result.window.is_onscreen);
    assert_eq!(result.screenshot.path, output.to_string_lossy());
    assert_eq!(
        result.screenshot.ocr_text.as_deref(),
        Some("Fixture content")
    );
    assert_eq!((result.image.width, result.image.height), (800, 600));
    assert_eq!((result.image.scale_x, result.image.scale_y), (2.0, 2.0));
    assert_eq!(result.image.origin, "window_frame_top_left");
    // A point in a Retina image maps to the correct global point on a left-hand display.
    assert_eq!(
        f64::from(result.window.bounds.x) + 200.0 / result.image.scale_x,
        -700.0
    );
    assert_eq!(
        f64::from(result.window.bounds.y) + 100.0 / result.image.scale_y,
        170.0
    );
    assert_eq!(
        fs::read(&output).expect("saved image"),
        png_header(800, 600)
    );
    assert_eq!(fs::read_dir(dir.path()).expect("list").count(), 1);
}

#[test]
fn changed_or_missing_window_does_not_replace_an_existing_output() {
    let original = window();
    let mut changed_pid = original.clone();
    changed_pid.owner_pid += 1;
    let mut changed_title = original.clone();
    changed_title.title.push('!');
    let mut moved = original.clone();
    moved.bounds.x += 100;
    let mut resized = original.clone();
    resized.bounds.width += 10;
    let mut changed_id = original.clone();
    changed_id.window_id += 1;

    for current in [
        Some(changed_pid),
        Some(changed_title),
        Some(moved),
        Some(resized),
        Some(changed_id),
        None,
    ] {
        let dir = tempfile::tempdir().expect("directory");
        let output = dir.path().join("preserved.png");
        fs::write(&output, b"original data").expect("original");
        let calls = Cell::new(0);
        let error = snapshot_with(
            42,
            false,
            output.to_str(),
            || {
                calls.set(calls.get() + 1);
                Ok(if calls.get() == 1 {
                    vec![original.clone()]
                } else {
                    current.clone().into_iter().collect()
                })
            },
            capture,
        )
        .expect_err("must reject a changed target");
        assert!(error.to_string().contains("window"), "{error}");
        assert_eq!(
            fs::read(&output).expect("original survives"),
            b"original data"
        );
        assert_eq!(fs::read_dir(dir.path()).expect("list").count(), 1);
    }
}

#[test]
fn missing_window_and_invalid_output_never_start_capture() {
    let dir = tempfile::tempdir().expect("directory");
    let output = dir.path().join("result.png");
    snapshot_with(
        99,
        false,
        output.to_str(),
        || Ok(vec![window()]),
        |_, _, _| panic!("missing target must not be captured"),
    )
    .expect_err("unknown id");
    assert!(!output.exists());
    snapshot_with(
        42,
        false,
        Some("../result.png"),
        || panic!("validate output first"),
        |_, _, _| panic!("invalid output must not be captured"),
    )
    .expect_err("invalid path");
}

#[test]
fn capture_failure_preserves_existing_output_and_cleans_temporary_file() {
    let dir = tempfile::tempdir().expect("directory");
    let output = dir.path().join("result.png");
    fs::write(&output, b"keep").expect("original");
    snapshot_with(
        42,
        false,
        output.to_str(),
        || Ok(vec![window()]),
        |_, path, _| {
            fs::write(path, b"partial capture").expect("partial");
            Err(MacosError::Other("capture unavailable".into()))
        },
    )
    .expect_err("capture failed");
    assert_eq!(fs::read(&output).expect("preserved"), b"keep");
    assert_eq!(fs::read_dir(dir.path()).expect("list").count(), 1);
}

#[test]
fn dimensions_reader_rejects_truncated_non_png_and_invalid_sizes() {
    let file = tempfile::NamedTempFile::new().expect("file");
    for header in [
        vec![],
        vec![0; 24],
        png_header(0, 600),
        png_header(800, 0),
        png_header(u32::MAX, 600),
    ] {
        fs::write(file.path(), header).expect("write header");
        assert!(read_image_metadata(file.path(), window().bounds).is_err());
    }
    fs::write(file.path(), png_header(800, 600)).expect("write header");
    let mut invalid_bounds = window().bounds;
    invalid_bounds.width = 0;
    assert!(read_image_metadata(file.path(), invalid_bounds).is_err());
}

#[test]
fn foreground_and_visibility_changes_do_not_change_window_identity() {
    let dir = tempfile::tempdir().expect("directory");
    let output = dir.path().join("result.png");
    let calls = Cell::new(0);
    let snapshot = snapshot_with(
        42,
        false,
        output.to_str(),
        || {
            calls.set(calls.get() + 1);
            let mut current = window();
            if calls.get() > 1 {
                current.is_onscreen = true;
                current.is_frontmost = true;
            }
            Ok(vec![current])
        },
        capture,
    )
    .expect("unchanged window frame");
    assert!(snapshot.window.is_onscreen);
    assert!(snapshot.window.is_frontmost);
}
