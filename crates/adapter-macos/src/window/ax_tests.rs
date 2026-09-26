use std::io::Write;
use std::process::Command;
use std::time::Duration;

#[test]
fn executable_binding_checks_offscreen_fallbacks_ambiguity_and_menu_context() {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").expect("Swift fixture");
    write!(
        script,
        "{}\n{}\n{}\n{}\n{}\ntestWindowBinding()\n",
        include_str!("ax_elements.swift"),
        include_str!("ax_shared.swift"),
        include_str!("ax_common.swift"),
        include_str!("ax_catalog_tests.swift"),
        include_str!("ax_binding_tests.swift")
    )
    .expect("write fixture");
    for (scenario, error) in [
        ("fallback", None),
        ("deduplicate", None),
        ("menu", None),
        ("missing", Some("0 AX windows")),
        ("ambiguous-ax", Some("2 AX windows")),
        ("ambiguous-catalog", Some("ambiguous in the system catalog")),
        ("other-main", Some("not the verified main window")),
        ("other-focused", Some("not the verified focused window")),
    ] {
        let output = super::process::run_with_timeout(
            Command::new("swift")
                .arg(script.path())
                .args(["123", "7", "Fixture", "-100", "200", "640", "480", scenario]),
            b"",
            Duration::from_secs(40),
        )
        .expect("run binding fixture");
        let stderr = String::from_utf8_lossy(&output.stderr);
        match error {
            Some(message) => {
                assert!(!output.status.success(), "{scenario} should be rejected");
                assert!(stderr.contains(message), "{scenario}: {stderr}");
            }
            None => {
                assert!(output.status.success(), "{scenario}: {stderr}");
                assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "passed");
            }
        }
    }
}

#[test]
fn catalog_binding_matches_snapshot_precision_and_rejects_changed_frames() {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift").expect("Swift fixture");
    write!(
        script,
        "{}\n{}\n{}\n{}\ntestCatalogBounds()\n",
        include_str!("ax_elements.swift"),
        include_str!("ax_shared.swift"),
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
