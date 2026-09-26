use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use super::snapshot_window;

struct Fixture {
    child: Child,
    directory: tempfile::TempDir,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Fixture {
    fn launch() -> Self {
        let directory = tempfile::tempdir().expect("fixture directory");
        let source = directory.path().join("fixture.swift");
        let executable = directory.path().join("SnapshotSheetFixture");
        fs::write(&source, include_str!("snapshot_sheet_fixture.swift")).expect("fixture source");
        let result = Command::new("swiftc")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("compile fixture");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let child = Command::new(executable)
            .arg(directory.path().join("state.json"))
            .arg(directory.path().join("attach"))
            .spawn()
            .expect("launch fixture");
        Self { child, directory }
    }

    fn state(&self, attached: bool) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let state = fs::read(self.directory.path().join("state.json"))
                .ok()
                .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok());
            if let Some(state) = state.filter(|state| state["attached"] == attached) {
                return state;
            }
            assert!(Instant::now() < deadline, "fixture state timed out");
            thread::sleep(Duration::from_millis(50));
        }
    }
}

fn dimensions(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).expect("PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(bytes[16..20].try_into().expect("width")),
        u32::from_be_bytes(bytes[20..24].try_into().expect("height")),
    )
}

#[test]
#[ignore = "requires a logged-in macOS desktop and Screen Recording permission"]
fn snapshot_excludes_attached_sheet_from_frame_coordinates() {
    let fixture = Fixture::launch();
    let id = fixture.state(false)["window_id"]
        .as_u64()
        .expect("window id") as u32;
    let plain_path = fixture.directory.path().join("plain.png");
    let plain = snapshot_window(id, false, plain_path.to_str()).expect("plain snapshot");

    fs::write(fixture.directory.path().join("attach"), b"attach").expect("attach sheet");
    let state = fixture.state(true);
    assert_eq!(
        state["extends_parent"], true,
        "sheet must extend beyond the parent frame"
    );

    // Exercise the real system default as a control, not a mock of command arguments.
    let combined_path = fixture.directory.path().join("combined.png");
    let combined = Command::new("screencapture")
        .args(["-x", "-t", "png", "-o", "-l", &id.to_string()])
        .arg(&combined_path)
        .output()
        .expect("system capture");
    assert!(
        combined.status.success(),
        "{}",
        String::from_utf8_lossy(&combined.stderr)
    );
    let parent_dimensions = dimensions(&plain_path);
    assert_ne!(
        dimensions(&combined_path),
        parent_dimensions,
        "control must include the attached sheet"
    );

    let frame_path = fixture.directory.path().join("frame.png");
    let frame = snapshot_window(id, false, frame_path.to_str()).expect("snapshot with sheet");
    assert_eq!(frame.window.bounds, plain.window.bounds);
    assert_eq!(
        dimensions(&frame_path),
        parent_dimensions,
        "attached sheet must not expand the image beyond the selected window frame"
    );
    assert_eq!((frame.image.width, frame.image.height), parent_dimensions);
    assert_eq!(
        (frame.image.scale_x, frame.image.scale_y),
        (plain.image.scale_x, plain.image.scale_y)
    );
}
