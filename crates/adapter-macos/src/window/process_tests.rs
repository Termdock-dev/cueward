use std::io::ErrorKind;
use std::process::Command;
use std::time::{Duration, Instant};

use super::run_with_timeout;

#[test]
fn transfers_large_input_and_captures_both_output_streams() {
    let payload = vec![b'x'; 512 * 1024];
    let output = run_with_timeout(
        Command::new("/bin/sh").args(["-c", "cat; printf diagnostic >&2"]),
        &payload,
        Duration::from_secs(5),
    )
    .expect("helper completes");
    assert!(output.status.success());
    assert_eq!(output.stdout, payload);
    assert_eq!(output.stderr, b"diagnostic");
}

#[test]
fn preserves_nonzero_exit_status_and_diagnostics() {
    let output = run_with_timeout(
        Command::new("/bin/sh").args(["-c", "printf failed >&2; exit 7"]),
        b"",
        Duration::from_secs(5),
    )
    .expect("helper exits");
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stderr, b"failed");
}

#[test]
fn bounds_startup_that_never_reads_its_input() {
    let start = Instant::now();
    let error = run_with_timeout(
        Command::new("/bin/sh").args(["-c", "exec sleep 10"]),
        &vec![b'x'; 512 * 1024],
        Duration::from_millis(150),
    )
    .expect_err("unresponsive driver must time out");
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(!error.to_string().contains("cleanup failed"), "{error}");
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn timeout_stops_descendants_before_they_can_perform_late_actions() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let started = directory.path().join("started");
    let late_action = directory.path().join("late-action");
    let error = run_with_timeout(
        Command::new("/bin/sh")
            .args([
                "-c",
                "(printf started > \"$1\"; sleep 1; printf wrong > \"$2\") & wait",
                "fixture",
            ])
            .arg(&started)
            .arg(&late_action),
        b"",
        Duration::from_millis(300),
    )
    .expect_err("driver must time out");
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(!error.to_string().contains("cleanup failed"), "{error}");
    assert!(started.exists(), "descendant must actually start");
    std::thread::sleep(Duration::from_secs(1));
    assert!(!late_action.exists(), "descendant survived timeout");
}
