use super::*;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

#[test]
fn finder_reveal_deadline_stops_worker_and_preserves_delivery_uncertainty() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("worker");
    std::fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let request = FinderRequest {
        root: directory.path().into(),
        action: FinderAction::Reveal {
            path: "a".into(),
            follow_links: false,
            expected_version: None,
        },
    };
    let start = Instant::now();
    let error = run(&executable, &request, 100).unwrap_err();
    assert_eq!(error.code, FileErrorCode::Timeout);
    assert!(error.message.contains("may have been delivered"));
    assert!(start.elapsed() < Duration::from_secs(5));
}
