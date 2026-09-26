use super::tests::Fixture;
use super::*;
use std::thread;
use std::time::Instant;

fn received(fixture: &Fixture, pid: i32, count: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    let path = fixture.directory.path().join(format!("state-{pid}.json"));
    loop {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(state) = serde_json::from_slice::<Value>(&bytes) {
                if state["documents"]
                    .as_array()
                    .is_some_and(|docs| docs.len() == count)
                {
                    assert_eq!(state["activations"], 0);
                    return state;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "document receiver did not report {count} files"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn assert_open_result(result: &OpenFileResult) {
    assert_eq!(result.status, OpenFileStatus::SentUnverified);
    assert!(!result.foreground_changed);
    assert!(!result.app.is_active);
}

#[test]
#[ignore = "requires an unlocked macOS desktop; opens files in disposable application instances"]
fn background_file_open_reaches_new_and_existing_instances_without_activation() {
    let fixture = Fixture::with_source(include_str!("open_live_fixture.swift"));
    let file = fixture.directory.path().join("document 測試.txt");
    fs::write(&file, "Cross-app Unicode αβγ\n測試\n").expect("document");
    let first = open_file(None, Some(&fixture.path), &file, false).expect("open new app");
    assert_open_result(&first);
    let first_state = received(&fixture, first.app.pid, 1);
    assert_eq!(
        first_state["documents"][0]["content"],
        "Cross-app Unicode αβγ\n測試\n"
    );
    assert_eq!(
        Path::new(first_state["documents"][0]["file"].as_str().expect("path"))
            .canonicalize()
            .expect("received file"),
        file.canonicalize().expect("file")
    );
    let second = open_file(None, Some(&fixture.path), &file, false).expect("open existing app");
    assert_open_result(&second);
    assert_eq!(second.app.pid, first.app.pid);
    received(&fixture, first.app.pid, 2);
    let separate =
        open_file(None, Some(&fixture.path), &file, true).expect("explicit new instance");
    assert_open_result(&separate);
    assert_ne!(separate.app.pid, first.app.pid);
    let separate_state = received(&fixture, separate.app.pid, 1);
    assert_eq!(separate_state["documents"], first_state["documents"]);
    let error =
        open_file(None, Some(&fixture.path), &file, false).expect_err("ambiguous instances");
    assert!(
        error.to_string().contains("multiple application instances"),
        "{error}"
    );
    assert_eq!(
        received(&fixture, first.app.pid, 2)["documents"]
            .as_array()
            .expect("documents")
            .len(),
        2
    );
}
