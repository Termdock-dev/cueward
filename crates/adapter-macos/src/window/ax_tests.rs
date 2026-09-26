use std::io::Write;
use std::process::Command;
use std::time::Duration;

#[test]
fn catalog_binding_matches_snapshot_precision_and_rejects_changed_frames() {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").expect("Swift fixture");
    write!(
        script,
        "{}\n{}\n{}\ntestCatalogBounds()\n",
        include_str!("ax_elements.swift"),
        include_str!("ax_common.swift"),
        include_str!("ax_catalog_tests.swift")
    )
    .expect("write fixture");
    let output = super::process::run_with_timeout(
        Command::new("swift")
            .arg(script.path())
            .args(["123", "7", "Fixture", "-100", "200", "640", "480"]),
        b"",
        Duration::from_secs(40),
    )
    .expect("run Swift fixture");
    assert!(
        output.status.success(),
        "catalog binding failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "passed");
}

#[test]
fn child_enumeration_distinguishes_leaves_from_failed_reads() {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").expect("Swift fixture");
    write!(
        script,
        "{}\n{}",
        include_str!("ax_elements.swift"),
        include_str!("ax_elements_tests.swift")
    )
    .expect("write fixture");
    let output = super::process::run_with_timeout(
        Command::new("swift").arg(script.path()),
        b"",
        Duration::from_secs(40),
    )
    .expect("run Swift fixture");
    assert!(
        output.status.success(),
        "AX element decoding failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "passed");
}
